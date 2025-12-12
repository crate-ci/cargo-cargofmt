use crate::toml::TokenKind;
use crate::toml::TomlToken;

#[tracing::instrument]
pub fn normalize_space_separators(tokens: &mut crate::toml::TomlTokens<'_>) {
    let mut indices = crate::toml::TokenIndices::new();
    while let Some(mut i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen | TokenKind::ArrayOpen => {
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get(next_i) {
                    if matches!(next.kind, TokenKind::Whitespace) {
                        tokens.tokens[next_i] = TomlToken::EMPTY;
                    }
                }
            }
            TokenKind::StdTableClose | TokenKind::ArrayTableClose | TokenKind::ArrayClose => {
                if let Some(prev_i) = i.checked_sub(1) {
                    if matches!(tokens.tokens[prev_i].kind, TokenKind::Whitespace) {
                        tokens.tokens[prev_i] = TomlToken::EMPTY;
                    }
                }
            }
            TokenKind::InlineTableOpen => {
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get(next_i) {
                    if matches!(next.kind, TokenKind::Whitespace) {
                        tokens.tokens[next_i] = TomlToken::SPACE;
                    } else if matches!(next.kind, TokenKind::SimpleKey) {
                        tokens.tokens.insert(next_i, TomlToken::SPACE);
                    }
                }
            }
            TokenKind::InlineTableClose => {
                if let Some(prev_i) = i.checked_sub(1) {
                    let prev = &tokens.tokens[prev_i];
                    if matches!(prev.kind, TokenKind::Whitespace) {
                        if prev_i
                            .checked_sub(1)
                            .map(|prev_prev_i| {
                                matches!(
                                    tokens.tokens[prev_prev_i].kind,
                                    TokenKind::InlineTableOpen
                                )
                            })
                            .unwrap_or(false)
                        {
                            tokens.tokens[prev_i] = TomlToken::EMPTY;
                        } else {
                            tokens.tokens[prev_i] = TomlToken::SPACE;
                        }
                    } else if matches!(prev.kind, TokenKind::Scalar | TokenKind::ValueSep) {
                        tokens.tokens.insert(i, TomlToken::SPACE);
                    }
                }
            }
            TokenKind::SimpleKey => {}
            TokenKind::KeySep => {
                if let Some(prev_i) = i.checked_sub(1) {
                    if matches!(tokens.tokens[prev_i].kind, TokenKind::Whitespace) {
                        tokens.tokens[prev_i] = TomlToken::EMPTY;
                    }
                }
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get(next_i) {
                    if matches!(next.kind, TokenKind::Whitespace) {
                        tokens.tokens[next_i] = TomlToken::EMPTY;
                    }
                }
            }
            TokenKind::KeyValSep => {
                if let Some(key_i) = indices.rev().skip(1).find(|i| {
                    !matches!(
                        tokens.tokens[*i].kind,
                        TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment
                    )
                }) {
                    let mut new_i = key_i + 1;
                    if matches!(tokens.tokens[new_i].kind, TokenKind::Whitespace) {
                        new_i += 1;
                    }
                    let token = tokens.tokens.remove(i);
                    tokens.tokens.insert(new_i, token);
                    indices.set_next_index(new_i + 1);
                    i = new_i;
                }
                if let Some(prev_i) = i.checked_sub(1) {
                    if matches!(tokens.tokens[prev_i].kind, TokenKind::Whitespace) {
                        tokens.tokens[prev_i] = TomlToken::SPACE;
                    } else if matches!(tokens.tokens[prev_i].kind, TokenKind::SimpleKey) {
                        tokens.tokens.insert(i, TomlToken::SPACE);
                    }
                }
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get(next_i) {
                    if matches!(next.kind, TokenKind::Whitespace) {
                        tokens.tokens[next_i] = TomlToken::SPACE;
                    } else if matches!(next.kind, TokenKind::Scalar) {
                        tokens.tokens.insert(next_i, TomlToken::SPACE);
                    }
                }
            }
            TokenKind::Scalar => {}
            TokenKind::ValueSep => {
                if let Some(value_i) = indices.rev().skip(1).find(|i| {
                    !matches!(
                        tokens.tokens[*i].kind,
                        TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment
                    )
                }) {
                    let mut new_i = value_i + 1;
                    if matches!(tokens.tokens[new_i].kind, TokenKind::Whitespace) {
                        new_i += 1;
                    }
                    let token = tokens.tokens.remove(i);
                    tokens.tokens.insert(new_i, token);
                    indices.set_next_index(new_i + 1);
                    i = new_i;
                }
                if let Some(prev_i) = i.checked_sub(1) {
                    if matches!(tokens.tokens[prev_i].kind, TokenKind::Whitespace) {
                        tokens.tokens[prev_i] = TomlToken::EMPTY;
                    }
                }
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get(next_i) {
                    if matches!(next.kind, TokenKind::Whitespace) {
                        tokens.tokens[next_i] = TomlToken::SPACE;
                    } else if matches!(next.kind, TokenKind::SimpleKey | TokenKind::Scalar) {
                        tokens.tokens.insert(next_i, TomlToken::SPACE);
                    }
                }
            }
            TokenKind::Whitespace => {}
            TokenKind::Comment => {
                if let Some(prev_i) = i.checked_sub(1) {
                    if matches!(tokens.tokens[prev_i].kind, TokenKind::Whitespace) {
                        tokens.tokens[prev_i] = TomlToken::SPACE;
                    } else if !matches!(tokens.tokens[prev_i].kind, TokenKind::Newline) {
                        tokens.tokens.insert(i, TomlToken::SPACE);
                    }
                }
            }
            TokenKind::Newline => {}
            TokenKind::Error => {}
        }
    }
    tokens.trim_empty_whitespace();
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::normalize_space_separators(&mut tokens);
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
    fn empty() {
        valid("", str![]);
    }

    #[test]
    fn key_value_without_spaces() {
        valid("key=5", str!["key = 5"]);
    }

    #[test]
    fn key_value_with_extra_spaces() {
        valid("key   =    5", str!["key = 5"]);
    }

    #[test]
    fn key_value_with_tab() {
        valid("key\t=\t5", str!["key = 5"]);
    }

    #[test]
    fn comment_without_spaces() {
        valid("key = 5#Hello", str!["key = 5 #Hello"]);
    }

    #[test]
    fn comment_with_extra_spaces() {
        valid("key = 5    #    Hello", str!["key = 5 #    Hello"]);
    }

    #[test]
    fn comment_with_tab() {
        valid("key = 5\t#\tHello", str!["key = 5 #	Hello"]);
    }

    #[test]
    fn array_empty() {
        valid("key = []", str!["key = []"]);
    }

    #[test]
    fn array_spaces() {
        valid("key = [    ]", str!["key = []"]);
    }

    #[test]
    fn array_tab() {
        valid("key = [\t]", str!["key = []"]);
    }

    #[test]
    fn array_value_without_spaces() {
        valid("key = [5]", str!["key = [5]"]);
    }

    #[test]
    fn array_value_with_extra_spaces() {
        valid("key = [    5    ]", str!["key = [5]"]);
    }

    #[test]
    fn array_value_with_tab() {
        valid("key = [\t5\t]", str!["key = [5]"]);
    }

    #[test]
    fn value_sep_without_spaces() {
        valid("key = [5,6]", str!["key = [5, 6]"]);
    }

    #[test]
    fn value_sep_with_extra_spaces() {
        valid("key = [5  ,  6]", str!["key = [5, 6]"]);
    }

    #[test]
    fn value_sep_with_tab() {
        valid("key = [5\t,\t6]", str!["key = [5, 6]"]);
    }

    #[test]
    fn value_sep_with_newline() {
        valid(
            "key = [5
,
6]",
            // TODO
            str![[r#"
key = [5,

6]
"#]],
        );
    }

    #[test]
    fn value_sep_with_comment() {
        valid(
            "key = [5 # hello
, # goodbye
6]",
            // TODO
            str![[r#"
key = [5, # hello
 # goodbye
6]
"#]],
        );
    }

    #[test]
    fn value_sep_trailing_without_spaces() {
        valid("key = [5,]", str!["key = [5,]"]);
    }

    #[test]
    fn value_sep_trailing_with_extra_spaces() {
        valid("key = [5  ,  ]", str!["key = [5,]"]);
    }

    #[test]
    fn value_sep_trailing_with_tab() {
        valid("key = [5\t,\t]", str!["key = [5,]"]);
    }

    #[test]
    fn inline_table_empty() {
        valid("key = {}", str!["key = {}"]);
    }

    #[test]
    fn inline_table_spaces() {
        valid("key = {    }", str!["key = {}"]);
    }

    #[test]
    fn inline_table_tab() {
        valid("key = {\t}", str!["key = {}"]);
    }

    #[test]
    fn inline_table_value_without_spaces() {
        valid("key = {key=5}", str!["key = { key = 5 }"]);
    }

    #[test]
    fn inline_table_value_with_extra_spaces() {
        valid("key = {    key     =    5    }", str!["key = { key = 5 }"]);
    }

    #[test]
    fn inline_table_value_with_tab() {
        valid("key = {\tkey\t=\t5\t}", str!["key = { key = 5 }"]);
    }

    #[test]
    fn inline_table_sep_without_spaces() {
        valid("key = {a=5,b=6}", str!["key = { a = 5, b = 6 }"]);
    }

    #[test]
    fn inline_table_sep_with_extra_spaces() {
        valid("key = {a=5    ,    b=6}", str!["key = { a = 5, b = 6 }"]);
    }

    #[test]
    fn inline_table_sep_with_tab() {
        valid("key = {a=5\t,\tb=6}", str!["key = { a = 5, b = 6 }"]);
    }

    #[test]
    fn table_without_spaces() {
        valid("[key]", str!["[key]"]);
    }

    #[test]
    fn table_with_extra_spaces() {
        valid("[   key    ]", str!["[key]"]);
    }

    #[test]
    fn table_with_tab() {
        valid("[\tkey\t]", str!["[key]"]);
    }

    #[test]
    fn array_of_tables_without_spaces() {
        valid("[[key]]", str!["[[key]]"]);
    }

    #[test]
    fn array_of_tables_with_extra_spaces() {
        valid("[[   key    ]]", str!["[[key]]"]);
    }

    #[test]
    fn array_of_tables_with_tab() {
        valid("[[\tkey\t]]", str!["[[key]]"]);
    }

    #[test]
    fn key_sep_without_spaces() {
        valid("a.b = 5", str!["a.b = 5"]);
    }

    #[test]
    fn key_sep_with_extra_spaces() {
        valid("a    .    b = 5", str!["a.b = 5"]);
    }

    #[test]
    fn key_sep_with_tab() {
        valid("a\t.\tb = 5", str!["a.b = 5"]);
    }
}
