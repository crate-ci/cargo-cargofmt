use crate::toml::Table;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

#[tracing::instrument(skip_all)]
pub fn sort_table_hierarchy(tokens: &mut TomlTokens<'_>) {
    let tables = Table::new(tokens);
    if tables.len() <= 1 {
        return;
    }

    let sorted = compute_sorted_order(&tables);

    if sorted.iter().enumerate().all(|(i, &j)| i == j) {
        return;
    }

    // span().start picks up any leading comment glued to the header.
    let section_starts: Vec<usize> = tables.iter().map(|t| t.span().start).collect();
    let preamble_end = section_starts[0];
    let token_count = tokens.len();

    let section_end =
        |i: usize| -> usize { section_starts.get(i + 1).copied().unwrap_or(token_count) };

    let mut pool: Vec<Option<TomlToken<'_>>> = tokens.tokens.drain(..).map(Some).collect();
    let mut new_tokens: Vec<TomlToken<'_>> = Vec::with_capacity(token_count);

    for slot in &mut pool[0..preamble_end] {
        new_tokens.push(slot.take().unwrap());
    }
    for &idx in &sorted {
        for slot in &mut pool[section_starts[idx]..section_end(idx)] {
            new_tokens.push(slot.take().unwrap());
        }
    }

    tokens.tokens = new_tokens;
}

fn compute_sorted_order(tables: &[Table]) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..tables.len()).collect();
    indices.sort_by_key(|&i| {
        let depth = tables[i].name().len();
        (effective_position(i, tables), depth, i)
    });
    indices
}

fn effective_position(idx: usize, tables: &[Table]) -> usize {
    let name = tables[idx].name();
    tables
        .iter()
        .enumerate()
        .filter(|(_, t)| name.starts_with(t.name()))
        .min_by_key(|(_, t)| t.name().len())
        .map(|(i, _)| i)
        .unwrap_or(idx)
}

#[cfg(test)]
mod test {
    use snapbox::IntoData;
    use snapbox::assert_data_eq;
    use snapbox::str;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::sort_table_hierarchy(&mut tokens);
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
    fn no_tables_only_key_values() {
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
    fn single_table_unchanged() {
        valid(
            r#"[package]
name = "foo"
"#,
            str![[r#"
[package]
name = "foo"

"#]],
        );
    }

    #[test]
    fn two_unrelated_tables_unchanged() {
        valid(
            r#"[package]
[dependencies]
"#,
            str![[r#"
[package]
[dependencies]

"#]],
        );
    }

    #[test]
    fn already_in_hierarchy_order_unchanged() {
        // already sorted, output should match input exactly.
        valid(
            r#"[package]
name = "foo"
[package.metadata]
bar = "baz"
[dependencies]
foo = "1.0"
"#,
            str![[r#"
[package]
name = "foo"
[package.metadata]
bar = "baz"
[dependencies]
foo = "1.0"

"#]],
        );
    }

    #[test]
    fn child_moved_before_unrelated_sibling() {
        // [package.metadata] ends up after [dependencies] and needs to move before it.
        valid(
            r#"[package]
name = "foo"
[dependencies]
foo = "1.0"
[package.metadata]
bar = "baz"
"#,
            str![[r#"
[package]
name = "foo"
[package.metadata]
bar = "baz"
[dependencies]
foo = "1.0"

"#]],
        );
    }

    #[test]
    fn multiple_children_moved_before_sibling() {
        // two [package] children got stuck after [dependencies]; both need to move before it.
        valid(
            r#"[package]
[dependencies]
foo = "1.0"
[package.metadata]
bar = "baz"
[package.lints]
rust = "warn"
"#,
            str![[r#"
[package]
[package.metadata]
bar = "baz"
[package.lints]
rust = "warn"
[dependencies]
foo = "1.0"

"#]],
        );
    }

    #[test]
    fn deep_hierarchy_sorted() {
        // [a.b] and [a.b.c] are under [a], so they both go before [b].
        valid(
            r#"[a]
[b]
[a.b]
[a.b.c]
"#,
            str![[r#"
[a]
[a.b]
[a.b.c]
[b]

"#]],
        );
    }

    #[test]
    fn child_before_parent_moved_after() {
        // [a.b] comes before [a] but should follow it.
        valid(
            r#"[a.b]
[a]
"#,
            str![[r#"
[a]
[a.b]

"#]],
        );
    }

    #[test]
    fn deeply_nested_out_of_order() {
        // [a.b.c] appears before [a.b]; [a.b] must come first.
        valid(
            r#"[a]
[a.b.c]
[a.b]
"#,
            str![[r#"
[a]
[a.b]
[a.b.c]

"#]],
        );
    }

    #[test]
    fn preamble_key_values_preserved() {
        // entries before the first table header stay at the top.
        valid(
            r#"key = "value"
[a]
[b]
[a.x]
"#,
            str![[r#"
key = "value"
[a]
[a.x]
[b]

"#]],
        );
    }

    #[test]
    fn leading_comment_travels_with_table() {
        // a leading comment moves with its table.
        valid(
            r#"[package]
# metadata section
[package.metadata]
bar = "baz"
# deps section
[dependencies]
foo = "1.0"
[package.lints]
rust = "warn"
"#,
            str![[r#"
[package]
# metadata section
[package.metadata]
bar = "baz"
[package.lints]
rust = "warn"
# deps section
[dependencies]
foo = "1.0"

"#]],
        );
    }
}
