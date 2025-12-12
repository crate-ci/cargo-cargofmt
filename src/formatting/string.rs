use std::borrow::Cow;

use toml_writer::TomlWrite as _;

use crate::toml::TokenKind;

#[tracing::instrument]
pub fn normalize_strings(tokens: &mut crate::toml::TomlTokens<'_>) {
    for i in tokens.indices() {
        let token = &mut tokens.tokens[i];
        match token.kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {}
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => {}
            TokenKind::StdTableClose | TokenKind::ArrayTableClose => {}
            TokenKind::ArrayClose | TokenKind::InlineTableClose => {}
            TokenKind::SimpleKey => {
                if token.encoding.is_some() {
                    let mut new_raw = String::new();
                    new_raw
                        .key(
                            toml_writer::TomlKeyBuilder::new(token.decoded.as_ref().unwrap())
                                .as_default(),
                        )
                        .unwrap();
                    token.raw = Cow::Owned(new_raw);
                }
            }
            TokenKind::KeySep => {}
            TokenKind::KeyValSep => {}
            TokenKind::Scalar => {
                if let Some(decoded) = token.decoded.as_ref() {
                    let mut new_raw = String::new();
                    new_raw
                        .value(toml_writer::TomlStringBuilder::new(decoded).as_default())
                        .unwrap();
                    token.raw = Cow::Owned(new_raw);
                }
            }
            TokenKind::ValueSep => {}
            TokenKind::Whitespace => {}
            TokenKind::Comment => {}
            TokenKind::Newline => {}
            TokenKind::Error => {}
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
        super::normalize_strings(&mut tokens);
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
    fn bare_key() {
        valid(
            r#"
a = "value"
"#,
            str![[r#"

a = "value"

"#]],
        );
    }

    #[test]
    fn normalize_key_basic() {
        valid(
            r#"
"" = "value"
"b" = "value"
"c'" = "value"
"d\"" = "value"
"#,
            str![[r#"

"" = "value"
b = "value"
"c'" = "value"
'd"' = "value"

"#]],
        );
    }

    #[test]
    fn normalize_key_literal() {
        valid(
            r#"
'' = "value"
'b' = "value"
'd"' = "value"
"#,
            str![[r#"

"" = "value"
b = "value"
'd"' = "value"

"#]],
        );
    }

    #[test]
    fn normalize_value_string_type() {
        valid(
            r#"
a = "value"
b = 'value'
c = """value"""
d = """
value"""
e = '''value'''
f = '''
value'''
"#,
            str![[r#"

a = "value"
b = "value"
c = "value"
d = "value"
e = "value"
f = "value"

"#]],
        );
    }

    #[test]
    fn normalize_value_escape() {
        valid(
            r#"
a = "a\"b"
b = 'a"b'
c = """a\"b"""
d = """
a\"b"""
e = '''a"b'''
f = '''
a"b'''
"#,
            str![[r#"

a = 'a"b'
b = 'a"b'
c = 'a"b'
d = 'a"b'
e = 'a"b'
f = 'a"b'

"#]],
        );
    }

    #[test]
    fn normalize_value_multi_leading_nl() {
        valid(
            r#"
a = """ab"""
b = """a\nb"""
c = """a
b
"""
d = """
a
b
"""
e = '''ab'''
f = '''a
b
'''
g = '''
a
b
'''
"#,
            str![[r#"

a = "ab"
b = """
a
b"""
c = """
a
b
"""
d = """
a
b
"""
e = "ab"
f = """
a
b
"""
g = """
a
b
"""

"#]],
        );
    }
}
