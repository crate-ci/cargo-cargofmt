use crate::config::lists::SeparatorTactic;

/// Assumptions:
/// - blocks are reflowed
#[tracing::instrument]
pub fn adjust_trailing_comma(_tokens: &mut crate::toml::TomlTokens<'_>, _tactic: SeparatorTactic) {}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    use super::SeparatorTactic;

    #[track_caller]
    fn valid(input: &str, tactic: SeparatorTactic, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::adjust_trailing_comma(&mut tokens, tactic);
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
    fn empty_always() {
        valid("", SeparatorTactic::Always, str![]);
    }

    #[test]
    fn empty_never() {
        valid("", SeparatorTactic::Never, str![]);
    }

    #[test]
    fn empty_vertical() {
        valid("", SeparatorTactic::Vertical, str![]);
    }

    #[test]
    fn array_always() {
        valid(
            "
empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]
",
            SeparatorTactic::Always,
            str![[r#"

empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]

"#]],
        );
    }

    #[test]
    fn array_never() {
        valid(
            "
empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]
",
            SeparatorTactic::Never,
            str![[r#"

empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]

"#]],
        );
    }

    #[test]
    fn array_vertical() {
        valid(
            "
empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]
",
            SeparatorTactic::Vertical,
            str![[r#"

empty = []

single-horizontal = [1]

single-horizontal-trailing = [1, ]

single-vertical = [
    1
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3,
]

"#]],
        );
    }

    #[test]
    fn inline_table_always() {
        valid(
            "
empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }
",
            SeparatorTactic::Always,
            str![[r#"

empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }

"#]],
        );
    }

    #[test]
    fn inline_table_never() {
        valid(
            "
empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }
",
            SeparatorTactic::Never,
            str![[r#"

empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }

"#]],
        );
    }

    #[test]
    fn inline_table_vertical() {
        valid(
            "
empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }
",
            SeparatorTactic::Vertical,
            str![[r#"

empty = {}

single-horizontal = { a = 1 }

multi-horizontal = { a = 1, b =  2, c = 3 }

"#]],
        );
    }
}
