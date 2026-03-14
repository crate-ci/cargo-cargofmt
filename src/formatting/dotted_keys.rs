use std::collections::HashMap;

use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

#[tracing::instrument(skip_all)]
pub fn sort_dotted_key_hierarchy(tokens: &mut TomlTokens<'_>) {
    let ranges = find_context_ranges(tokens);

    let orderings: Vec<(Vec<Entry>, Vec<usize>)> = ranges
        .iter()
        .map(|r| {
            let entries = collect_entries(tokens, r.clone());
            let order = compute_sorted_order(&entries);
            (entries, order)
        })
        .collect();

    if orderings
        .iter()
        .all(|(_, order)| order.iter().enumerate().all(|(i, &j)| i == j))
    {
        return;
    }

    let mut new_tokens: Vec<TomlToken<'_>> = Vec::with_capacity(tokens.tokens.len());
    let mut pos = 0;

    for (range, (entries, order)) in ranges.iter().zip(orderings.iter()) {
        // header and any gap before this section
        new_tokens.extend_from_slice(&tokens.tokens[pos..range.start]);

        for &idx in order {
            let e = &entries[idx];
            new_tokens.extend_from_slice(&tokens.tokens[e.start..e.end]);
        }

        // trailing blank lines stay at the end
        let tail = entries.last().map_or(range.start, |e| e.end);
        new_tokens.extend_from_slice(&tokens.tokens[tail..range.end]);

        pos = range.end;
    }

    new_tokens.extend_from_slice(&tokens.tokens[pos..]);
    tokens.tokens = new_tokens;
}

fn find_context_ranges(tokens: &TomlTokens<'_>) -> Vec<std::ops::Range<usize>> {
    let n = tokens.tokens.len();
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < n {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {
                if start < i {
                    ranges.push(start..i);
                }
                // skip the rest of the header line
                while i < n && tokens.tokens[i].kind != TokenKind::Newline {
                    i += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if start < n {
        ranges.push(start..n);
    }

    ranges
}

struct Entry {
    start: usize,
    end: usize,
    root: String,
}

fn collect_entries(tokens: &TomlTokens<'_>, context: std::ops::Range<usize>) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut entry_start = context.start;
    let mut key: Option<usize> = None;
    let mut depth: u32 = 0;
    let mut i = context.start;

    while i < context.end {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => {
                depth = depth.saturating_sub(1);
            }
            // depth > 0 means inside a value; key.is_some() means mid-statement
            TokenKind::SimpleKey if depth == 0 && key.is_none() => {
                key = Some(i);
            }
            TokenKind::Newline if depth == 0 => {
                if let Some(k) = key.take() {
                    let root = tokens.tokens[k].decoded.as_deref().unwrap_or("").to_owned();
                    entries.push(Entry {
                        start: entry_start,
                        end: i + 1,
                        root,
                    });
                    entry_start = i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    // no trailing newline
    if let Some(k) = key {
        let root = tokens.tokens[k].decoded.as_deref().unwrap_or("").to_owned();
        entries.push(Entry {
            start: entry_start,
            end: context.end,
            root,
        });
    }

    entries
}

fn compute_sorted_order(entries: &[Entry]) -> Vec<usize> {
    let mut first: HashMap<&str, usize> = HashMap::new();
    for (i, e) in entries.iter().enumerate() {
        first.entry(e.root.as_str()).or_insert(i);
    }
    let mut order: Vec<usize> = (0..entries.len()).collect();
    order.sort_by_key(|&i| (*first.get(entries[i].root.as_str()).unwrap_or(&i), i));
    order
}

#[cfg(test)]
mod test {
    use snapbox::IntoData;
    use snapbox::assert_data_eq;
    use snapbox::str;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::sort_dotted_key_hierarchy(&mut tokens);
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
    fn no_dotted_keys() {
        valid(
            r#"name = "foo"
version = "1.0"
"#,
            str![[r#"
name = "foo"
version = "1.0"

"#]],
        );
    }

    #[test]
    fn already_grouped() {
        // already in order, must not be modified
        valid(
            r#"apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"
"#,
            str![[r#"
apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"

"#]],
        );
    }

    #[test]
    fn basic_grouping() {
        // core case from issue #56
        valid(
            r#"apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"

"#]],
        );
    }

    #[test]
    fn three_groups_interleaved() {
        valid(
            "a.x = 1
b.x = 2
c.x = 3
a.y = 4
b.y = 5
c.y = 6
",
            str![[r#"
a.x = 1
a.y = 4
b.x = 2
b.y = 5
c.x = 3
c.y = 6

"#]],
        );
    }

    #[test]
    fn inside_table_section() {
        valid(
            r#"[fruits]
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
[fruits]
apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"

"#]],
        );
    }

    #[test]
    fn multiple_sections_sorted_independently() {
        valid(
            "[a]
x.foo = 1
y.foo = 2
x.bar = 3
[b]
p.foo = 1
q.foo = 2
p.bar = 3
",
            str![[r#"
[a]
x.foo = 1
x.bar = 3
y.foo = 2
[b]
p.foo = 1
p.bar = 3
q.foo = 2

"#]],
        );
    }

    #[test]
    fn plain_key_pushed_after_its_group() {
        // plain key between two apple entries ends up after them
        valid(
            r#"apple.type = "fruit"
version = "1.0"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
apple.skin = "thin"
version = "1.0"

"#]],
        );
    }

    #[test]
    fn comment_travels_with_following_entry() {
        // comment before orange moves with it
        valid(
            r#"apple.type = "fruit"
# the orange entry
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
apple.skin = "thin"
# the orange entry
orange.type = "citrus"

"#]],
        );
    }

    #[test]
    fn multiline_array_value() {
        // multi-line value is still one statement
        valid(
            r#"apple.colors = [
  "red",
  "green",
]
orange.colors = ["orange"]
apple.type = "fruit"
"#,
            str![[r#"
apple.colors = [
  "red",
  "green",
]
apple.type = "fruit"
orange.colors = ["orange"]

"#]],
        );
    }

    #[test]
    fn inline_table_value() {
        // keys inside an inline table value are not statement starts
        valid(
            r#"apple.info = { type = "fruit", color = "red" }
orange.info = { type = "citrus" }
apple.name = "Apple"
"#,
            str![[r#"
apple.info = { type = "fruit", color = "red" }
apple.name = "Apple"
orange.info = { type = "citrus" }

"#]],
        );
    }

    #[test]
    fn preamble_key_values_sorted() {
        // preamble before the first header is a context too
        valid(
            "x.a = 1
y.a = 2
x.b = 3
[section]
",
            str![[r#"
x.a = 1
x.b = 3
y.a = 2
[section]

"#]],
        );
    }

    #[test]
    fn single_entry_unchanged() {
        valid(
            "foo.bar = 1
",
            str![[r#"
foo.bar = 1

"#]],
        );
    }

    #[test]
    fn no_shared_roots_unchanged() {
        valid(
            "a.x = 1
b.x = 2
c.x = 3
",
            str![[r#"
a.x = 1
b.x = 2
c.x = 3

"#]],
        );
    }

    #[test]
    fn empty_section_body_unchanged() {
        // section with no key-value pairs is a no-op
        valid(
            "[a]
[b]
key = 1
",
            str![[r#"
[a]
[b]
key = 1

"#]],
        );
    }

    #[test]
    fn array_of_tables_section() {
        valid(
            "[[targets]]
foo.x = 1
bar.x = 2
foo.y = 3
",
            str![[r#"
[[targets]]
foo.x = 1
foo.y = 3
bar.x = 2

"#]],
        );
    }
}
