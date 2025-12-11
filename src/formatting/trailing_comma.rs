use crate::config::lists::SeparatorTactic;
use crate::toml::TokenKind;
use crate::toml::TomlToken;

/// Assumptions:
/// - blocks are reflowed
#[tracing::instrument]
pub fn adjust_trailing_comma(tokens: &mut crate::toml::TomlTokens<'_>, tactic: SeparatorTactic) {
    let mut indices = crate::toml::TokenIndices::new();
    while let Some(mut i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {}
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => {}
            TokenKind::StdTableClose | TokenKind::ArrayTableClose => {}
            TokenKind::ArrayClose => {
                if let Some(prev_i) = indices.rev().skip(1).find(|i| {
                    matches!(
                        tokens.tokens[*i].kind,
                        TokenKind::ValueSep
                            | TokenKind::Scalar
                            | TokenKind::ArrayClose
                            | TokenKind::InlineTableClose
                    )
                }) {
                    let prev_kind = tokens.tokens[prev_i].kind;
                    let action = match (tactic, prev_kind) {
                        (SeparatorTactic::Always, TokenKind::ValueSep) => None,
                        (SeparatorTactic::Always, _) => Some(Action::Add),
                        (SeparatorTactic::Never, TokenKind::ValueSep) => Some(Action::Remove),
                        (SeparatorTactic::Never, _) => None,
                        (SeparatorTactic::Vertical, TokenKind::ValueSep) => {
                            if tokens.tokens[prev_i..i]
                                .iter()
                                .any(|t| t.kind == TokenKind::Newline)
                            {
                                None
                            } else {
                                Some(Action::Remove)
                            }
                        }
                        (SeparatorTactic::Vertical, _) => {
                            if tokens.tokens[prev_i..i]
                                .iter()
                                .any(|t| t.kind == TokenKind::Newline)
                            {
                                Some(Action::Add)
                            } else {
                                None
                            }
                        }
                    };
                    match action {
                        Some(Action::Add) => {
                            tokens.tokens.insert(prev_i + 1, TomlToken::VAL_SEP);
                            i += 1;
                            indices.reset(i + 1);
                        }
                        Some(Action::Remove) => {
                            tokens.tokens.remove(prev_i);
                            i -= 1;
                            indices.reset(i + 1);
                            if tokens.tokens[prev_i].kind == TokenKind::Whitespace {
                                tokens.tokens.remove(prev_i);
                                i -= 1;
                                indices.reset(i + 1);
                            }
                        }
                        None => {}
                    }
                }
            }
            TokenKind::InlineTableClose => {}
            TokenKind::SimpleKey => {}
            TokenKind::KeySep => {}
            TokenKind::KeyValSep => {}
            TokenKind::Scalar => {}
            TokenKind::ValueSep => {}
            TokenKind::Whitespace => {}
            TokenKind::Comment => {}
            TokenKind::Newline => {}
            TokenKind::Error => {}
        }
    }
}

enum Action {
    Add,
    Remove,
}

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

single-horizontal = [1,]

single-horizontal-trailing = [1, ]

single-vertical = [
    1,
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3,]

multi-horizontal-trailing = [1, 2, 3, ]

multi-vertical = [
    1,
    2,
    3,
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

single-horizontal-trailing = [1]

single-vertical = [
    1
]

single-vertical-trailing = [
    1
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3]

multi-vertical = [
    1,
    2,
    3
]

multi-vertical-trailing = [
    1,
    2,
    3
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

single-horizontal-trailing = [1]

single-vertical = [
    1,
]

single-vertical-trailing = [
    1,
]

multi-horizontal = [1, 2, 3]

multi-horizontal-trailing = [1, 2, 3]

multi-vertical = [
    1,
    2,
    3,
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
