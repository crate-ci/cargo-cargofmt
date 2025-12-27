use std::collections::HashSet;

use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

struct TableInfo {
    name: Vec<String>,
    is_array_table: bool,
    header_start: usize,
    header_end: usize,
    has_content: bool,
    has_comment: bool,
}

#[tracing::instrument]
pub fn remove_unused_parent_tables(tokens: &mut TomlTokens<'_>) {
    // Pass 1: Collect information about all tables
    let tables = collect_table_info(tokens);

    if tables.is_empty() {
        return;
    }

    // Build set of table names that have children
    let parent_names = find_parent_names(&tables);

    // Pass 2: Remove empty, uncommented standard tables that have children
    for table in tables.iter().rev() {
        if should_remove(table, &parent_names) {
            for i in table.header_start..=table.header_end {
                tokens.tokens[i] = TomlToken::EMPTY;
            }
        }
    }

    tokens.trim_empty_whitespace();
}

fn collect_table_info(tokens: &TomlTokens<'_>) -> Vec<TableInfo> {
    let mut tables = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let kind = tokens.tokens[i].kind;

        if matches!(kind, TokenKind::StdTableOpen | TokenKind::ArrayTableOpen) {
            let is_array_table = kind == TokenKind::ArrayTableOpen;
            let header_start = i;

            // Parse the table name
            let (name, close_idx) = parse_table_name(tokens, i + 1, is_array_table);

            // Find the end of the header line (newline) and check for comment
            let (header_end, has_comment) = find_header_end(tokens, close_idx);

            // Check if the table has content (key-value pairs before next table)
            let has_content = check_has_content(tokens, header_end + 1);

            tables.push(TableInfo {
                name,
                is_array_table,
                header_start,
                header_end,
                has_content,
                has_comment,
            });

            i = header_end + 1;
        } else {
            i += 1;
        }
    }

    tables
}

fn parse_table_name(tokens: &TomlTokens<'_>, start: usize, is_array: bool) -> (Vec<String>, usize) {
    let close_kind = if is_array {
        TokenKind::ArrayTableClose
    } else {
        TokenKind::StdTableClose
    };

    let mut name = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        match tokens.tokens[i].kind {
            TokenKind::SimpleKey => {
                let token = &tokens.tokens[i];
                name.push(token.decoded.as_ref().unwrap_or(&token.raw).to_string());
            }
            TokenKind::KeySep | TokenKind::Whitespace => {}
            k if k == close_kind => {
                return (name, i);
            }
            _ => break,
        }
        i += 1;
    }

    (name, i)
}

fn find_header_end(tokens: &TomlTokens<'_>, close_idx: usize) -> (usize, bool) {
    let mut has_comment = false;
    let mut i = close_idx + 1;

    while i < tokens.len() {
        match tokens.tokens[i].kind {
            TokenKind::Comment => {
                has_comment = true;
            }
            TokenKind::Newline => {
                return (i, has_comment);
            }
            TokenKind::Whitespace => {}
            _ => break,
        }
        i += 1;
    }

    // End of file without newline
    (tokens.len() - 1, has_comment)
}

fn check_has_content(tokens: &TomlTokens<'_>, start: usize) -> bool {
    let mut i = start;

    while i < tokens.len() {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {
                return false;
            }
            TokenKind::SimpleKey => {
                // Look ahead for KeyValSep to confirm this is a key-value pair
                for j in (i + 1)..tokens.len() {
                    match tokens.tokens[j].kind {
                        TokenKind::KeyValSep => return true,
                        TokenKind::KeySep | TokenKind::Whitespace => {}
                        _ => break,
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }

    false
}

fn find_parent_names(tables: &[TableInfo]) -> HashSet<Vec<String>> {
    tables
        .iter()
        .flat_map(|t| (1..t.name.len()).map(|len| t.name[..len].to_vec()))
        .collect()
}

fn should_remove(table: &TableInfo, parent_names: &HashSet<Vec<String>>) -> bool {
    !table.is_array_table
        && !table.has_content
        && !table.has_comment
        && parent_names.contains(&table.name)
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::remove_unused_parent_tables(&mut tokens);
        let actual = tokens.to_string();

        assert_data_eq!(&actual, expected);

        let (_, errors) = toml::de::DeTable::parse_recoverable(&actual);
        if !errors.is_empty() {
            use std::fmt::Write as _;
            let mut result = String::new();
            writeln!(&mut result, "---").unwrap();
            for error in errors {
                writeln!(&mut result, "{error}").unwrap();
                writeln!(&mut result, "---").unwrap();
            }
            panic!("failed to parse\n---\n{actual}\n{result}");
        }
    }

    #[test]
    fn empty_input() {
        valid("", str![]);
    }

    #[test]
    fn remove_empty_parent_without_comment() {
        valid(
            "[parent]
[parent.child]
key = 1
",
            str![[r#"
[parent.child]
key = 1

"#]],
        );
    }

    #[test]
    fn remove_multiple_empty_parents() {
        valid(
            "[a]
[a.b]
[x]
[x.y]
",
            str![[r#"
[a.b]
[x.y]

"#]],
        );
    }

    #[test]
    fn remove_deeply_nested_empty_parents() {
        valid(
            "[a]
[a.b]
[a.b.c]
key = 1
",
            str![[r#"
[a.b.c]
key = 1

"#]],
        );
    }

    #[test]
    fn preserve_parent_with_trailing_comment() {
        valid(
            "[parent] # important section
[parent.child]
key = 1
",
            str![[r#"
[parent] # important section
[parent.child]
key = 1

"#]],
        );
    }

    #[test]
    fn preserve_parent_with_comment_no_space() {
        valid(
            "[parent]# comment
[parent.child]
",
            str![[r#"
[parent]# comment
[parent.child]

"#]],
        );
    }

    #[test]
    fn preserve_parent_with_content() {
        valid(
            r#"[parent]
key = "value"
[parent.child]
other = 1
"#,
            str![[r#"
[parent]
key = "value"
[parent.child]
other = 1

"#]],
        );
    }

    #[test]
    fn preserve_standalone_table_no_children() {
        valid(
            "[standalone]
",
            str![[r#"
[standalone]

"#]],
        );
    }

    #[test]
    fn remove_empty_parent_of_array_table() {
        valid(
            r#"[servers]
[[servers.production]]
ip = "10.0.0.1"
"#,
            str![[r#"
[[servers.production]]
ip = "10.0.0.1"

"#]],
        );
    }

    #[test]
    fn preserve_array_table_parent_with_comment() {
        valid(
            r#"[servers] # Server configurations
[[servers.production]]
ip = "10.0.0.1"
"#,
            str![[r#"
[servers] # Server configurations
[[servers.production]]
ip = "10.0.0.1"

"#]],
        );
    }

    #[test]
    fn preserve_array_table_even_if_empty() {
        valid(
            "[[a]]
[[a.b]]
key = 1
",
            str![[r#"
[[a]]
[[a.b]]
key = 1

"#]],
        );
    }

    #[test]
    fn mixed_parent_child_relationships() {
        valid(
            "[a]
[a.b]
key = 1

[c]
value = 2

[d]
[d.e]
[d.e.f]
deep = 3
",
            str![[r#"
[a.b]
key = 1

[c]
value = 2

[d.e.f]
deep = 3

"#]],
        );
    }

    #[test]
    fn sibling_tables_not_affected() {
        valid(
            "[a.b]
x = 1
[a.c]
y = 2
",
            str![[r#"
[a.b]
x = 1
[a.c]
y = 2

"#]],
        );
    }

    #[test]
    fn only_key_values_no_tables() {
        valid(
            r#"key = "value"
other = 123
"#,
            str![[r#"
key = "value"
other = 123

"#]],
        );
    }

    #[test]
    fn quoted_table_names() {
        valid(
            r#"["quoted"]
["quoted".child]
"#,
            str![[r#"
["quoted".child]

"#]],
        );
    }

    #[test]
    fn preserve_blank_lines_between_remaining_tables() {
        valid(
            "[a]
[a.b]

[c]
[c.d]
",
            str![[r#"
[a.b]

[c.d]

"#]],
        );
    }

    #[test]
    fn parent_between_children() {
        valid(
            "[a.first]
x = 1
[a]
[a.second]
y = 2
",
            str![[r#"
[a.first]
x = 1
[a.second]
y = 2

"#]],
        );
    }
}
