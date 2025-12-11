use crate::toml::TomlToken;
use std::borrow::Cow;

#[tracing::instrument]
pub fn trim_trailing_spaces(tokens: &mut crate::toml::TomlTokens<'_>) {
    if let Some(last) = tokens.tokens.last_mut() {
        if last.kind == crate::toml::TokenKind::Whitespace {
            *last = TomlToken::EMPTY;
        }
    }
    for i in tokens.indices() {
        if tokens.tokens[i].kind != crate::toml::TokenKind::Newline {
            continue;
        }
        let Some(prev_i) = i.checked_sub(1) else {
            continue;
        };
        if tokens.tokens[prev_i].kind != crate::toml::TokenKind::Whitespace {
            continue;
        }
        tokens.tokens[prev_i] = TomlToken::EMPTY;
    }
    tokens.trim_empty_whitespace();

    for i in tokens.indices() {
        if tokens.tokens[i].kind != crate::toml::TokenKind::Comment {
            continue;
        }
        tokens.tokens[i].raw = match std::mem::take(&mut tokens.tokens[i].raw) {
            Cow::Borrowed(s) => Cow::Borrowed(s.trim_end()),
            Cow::Owned(mut s) => {
                let trimmed = s.trim_end();
                s.replace_range(0..trimmed.len(), "");
                Cow::Owned(s)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::trim_trailing_spaces(&mut tokens);
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
    fn whitespace() {
        valid("    ", str![""]);
    }

    #[test]
    fn leading_newline() {
        valid(
            "\n  \n  ",
            str![[r#"



"#]],
        );
    }

    #[test]
    fn trailing_newline() {
        valid(
            "  \n  \n",
            str![[r#"



"#]],
        );
    }

    #[test]
    fn trailing() {
        valid(
            r#"
  
after_value = "value"  
after_comment = "value"  # Hello  
[after_table]  
after_array_bits = [  
  1  ,  
  2  ,  
]  
"#,
            str![[r#"


after_value = "value"
after_comment = "value"  # Hello
[after_table]
after_array_bits = [
  1  ,
  2  ,
]

"#]],
        );
    }
}
