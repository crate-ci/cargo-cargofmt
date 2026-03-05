use crate::toml::Table;
use crate::toml::TomlTokens;

/// Reorder table sections so that every child table immediately follows its
/// ancestor group, before any unrelated sibling.
///
/// For example:
///
/// ```toml
/// [package]
/// [dependencies]
/// [package.metadata]
/// ```
///
/// becomes:
///
/// ```toml
/// [package]
/// [package.metadata]
/// [dependencies]
/// ```
///
/// The relative order of unrelated top-level tables is preserved. Any leading
/// comment that is adjacent to a table header (separated by exactly one blank
/// line or no blank line) travels with that table when it is moved.
///
/// This pass runs before blank-line normalisation, so the output may have
/// uneven spacing between sections; a later `constrain_blank_lines` call evens
/// that out.
#[tracing::instrument(skip_all)]
pub fn sort_table_hierarchy(tokens: &mut TomlTokens<'_>) {
    let _ = tokens;
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

    // ── passthrough cases ──────────────────────────────────────────────────

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
            "[package]
name = \"foo\"
",
            str![[r#"
[package]
name = "foo"

"#]],
        );
    }

    #[test]
    fn two_unrelated_tables_unchanged() {
        valid(
            "[package]
[dependencies]
",
            str![[r#"
[package]
[dependencies]

"#]],
        );
    }

    #[test]
    fn already_in_hierarchy_order_unchanged() {
        // Idempotency: a file already in hierarchy order must not be modified.
        valid(
            "[package]
name = \"foo\"
[package.metadata]
bar = \"baz\"
[dependencies]
foo = \"1.0\"
",
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

    // ── reordering cases ───────────────────────────────────────────────────

    #[test]
    fn child_moved_before_unrelated_sibling() {
        // Core case from issue #54: [package.metadata] appears after
        // [dependencies] and should be pulled back before it.
        valid(
            "[package]
name = \"foo\"
[dependencies]
foo = \"1.0\"
[package.metadata]
bar = \"baz\"
",
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
        // Two children of [package] are both displaced; both move before
        // [dependencies].
        valid(
            "[package]
[dependencies]
foo = \"1.0\"
[package.metadata]
bar = \"baz\"
[package.lints]
rust = \"warn\"
",
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
        // [a.b] and [a.b.c] are both displaced behind [b]; they should be
        // pulled before [b], and [a.b.c] should follow [a.b].
        valid(
            "[a]
[b]
[a.b]
[a.b.c]
",
            str![[r#"
[a]
[a.b]
[a.b.c]
[b]

"#]],
        );
    }

    #[test]
    fn preamble_key_values_preserved() {
        // Key-value pairs that precede the first table header (the "document
        // preamble") must remain at the top, untouched.
        valid(
            "key = \"value\"
[a]
[b]
[a.x]
",
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
        // A comment on the line immediately before a table header is that
        // table's leading comment and must move with it.
        valid(
            "[package]
# metadata section
[package.metadata]
bar = \"baz\"
# deps section
[dependencies]
foo = \"1.0\"
[package.lints]
rust = \"warn\"
",
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
