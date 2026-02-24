use std::collections::HashSet;

use crate::toml::Table;
use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

#[tracing::instrument]
pub fn remove_unused_parent_tables(tokens: &mut TomlTokens<'_>) {
    let tables = Table::new(tokens);

    if tables.is_empty() {
        return;
    }

    let parent_names = find_parent_names(&tables);

    for table in tables.iter().rev() {
        if should_remove(table, tokens, &parent_names) {
            for i in table.start..table.end {
                tokens.tokens[i] = TomlToken::EMPTY;
            }
        }
    }

    tokens.trim_empty_whitespace();
}

fn find_parent_names(tables: &[Table]) -> HashSet<Vec<String>> {
    tables
        .iter()
        .flat_map(|t| (1..t.name.len()).map(|len| t.name[..len].to_vec()))
        .collect()
}

fn should_remove(
    table: &Table,
    tokens: &TomlTokens<'_>,
    parent_names: &HashSet<Vec<String>>,
) -> bool {
    if table.is_array_table {
        return false;
    }

    if !parent_names.contains(&table.name) {
        return false;
    }

    !has_body(tokens, table.start, table.end)
}

fn has_body(tokens: &TomlTokens<'_>, start: usize, end: usize) -> bool {
    let mut in_header = false;

    for i in start..end {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {
                in_header = true;
            }
            TokenKind::StdTableClose | TokenKind::ArrayTableClose => {
                in_header = false;
            }
            TokenKind::Whitespace | TokenKind::Newline => {}
            _ if !in_header => {
                return true;
            }
            _ => {}
        }
    }

    false
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

    #[test]
    fn parent_after_child() {
        valid(
            "[parent.child]

[parent]
",
            str![[r#"
[parent.child]


"#]],
        );
    }

    #[test]
    fn child_precedes_parent_adjacent_comment() {
        valid(
            "[parent.child]
key = 1
# comment
[parent]
",
            str![[r#"
[parent.child]
key = 1
# comment
[parent]

"#]],
        );
    }

    #[test]
    fn child_precedes_parent_body_comment() {
        valid(
            "[parent.child]
key = 1
# comment

[parent]
",
            str![[r#"
[parent.child]
key = 1
# comment


"#]],
        );
    }

    #[test]
    fn leading_comment_before_parent() {
        valid(
            "# leading comment
[parent]
[parent.child]
",
            str![[r#"
# leading comment
[parent]
[parent.child]

"#]],
        );
    }

    #[test]
    fn body_comment_blank_line_before_parent() {
        valid(
            "# body comment

[parent]
[parent.child]
",
            str![[r#"
# body comment

[parent.child]

"#]],
        );
    }

    #[test]
    fn leading_comment_after_other_table() {
        valid(
            "[other]

# leading comment
[parent]
[parent.child]
",
            str![[r#"
[other]

# leading comment
[parent]
[parent.child]

"#]],
        );
    }

    #[test]
    fn ambiguous_comment_after_other_table() {
        valid(
            "[other]
# ambiguous comment
[parent]
[parent.child]
",
            str![[r#"
[other]
# ambiguous comment
[parent]
[parent.child]

"#]],
        );
    }

    #[test]
    fn body_comment_blank_lines_after_other_table() {
        valid(
            "[other]

# body comment

[parent]
[parent.child]
",
            str![[r#"
[other]

# body comment

[parent.child]

"#]],
        );
    }

    #[test]
    fn detached_body_comment_before_other_table() {
        valid(
            "[parent]

# comment about parent fields

[other]
[parent.child]
",
            str![[r#"
[parent]

# comment about parent fields

[other]
[parent.child]

"#]],
        );
    }

    #[test]
    fn body_comment_after_parent_header() {
        valid(
            "[parent]
# body comment

[parent.child]
",
            str![[r#"
[parent]
# body comment

[parent.child]

"#]],
        );
    }

    #[test]
    fn body_comment_blank_line_after_parent_header() {
        valid(
            "[parent]

# body comment

[parent.child]
",
            str![[r#"
[parent]

# body comment

[parent.child]

"#]],
        );
    }

    #[test]
    fn ambiguous_comment_after_parent_header() {
        valid(
            "[parent]
# ambiguous comment
[parent.child]
",
            str![[r#"
[parent]
# ambiguous comment
[parent.child]

"#]],
        );
    }

    #[test]
    fn whitespace_between_parent_and_child() {
        valid(
            "[parent]

[parent.child]
",
            str![[r#"
[parent.child]

"#]],
        );
    }

    #[test]
    fn whitespace_on_blank_line_between_parent_and_child() {
        valid(
            "[parent]\n    \n[parent.child]\n",
            str![[r#"
[parent.child]

"#]],
        );
    }

    #[test]
    fn leading_comment_for_child_after_blank_line() {
        // Comment is adjacent to child, so included in child's start.
        // But also within parent's end range, so parent is preserved.
        valid(
            "[parent]

# leading comment for child
[parent.child]
key = 1
",
            str![[r#"
[parent]

# leading comment for child
[parent.child]
key = 1

"#]],
        );
    }

    #[test]
    fn eof_without_trailing_newline() {
        valid(
            "[parent]
[parent.child]
key = 1",
            str![[r#"
[parent.child]
key = 1
"#]],
        );
    }
}
