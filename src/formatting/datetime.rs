use std::borrow::Cow;

use crate::toml::ScalarKind;
use crate::toml::TokenKind;

/// Normalizes datetime separators to use `T` instead of space or lowercase `t`.
///
/// TOML allows `2025-12-26T10:30:00`, `2025-12-26t10:30:00`, and `2025-12-26 10:30:00`.
/// This function normalizes to the uppercase `T` form for consistency.
#[tracing::instrument]
pub fn normalize_datetime_separators(tokens: &mut crate::toml::TomlTokens<'_>) {
    // YYYY-MM-DD is 10 characters, so the separator is at index 10
    const DATE_LEN: usize = 10;

    for i in tokens.indices() {
        let token = &mut tokens.tokens[i];
        if token.kind == TokenKind::Scalar && token.scalar == Some(ScalarKind::DateTime) {
            let raw_bytes = token.raw.as_bytes();
            if raw_bytes.len() > DATE_LEN && matches!(raw_bytes[DATE_LEN], b' ' | b't') {
                let mut new_raw = token.raw.to_string();
                new_raw.replace_range(DATE_LEN..=DATE_LEN, "T");
                token.raw = Cow::Owned(new_raw);
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
        super::normalize_datetime_separators(&mut tokens);
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
    fn no_datetime() {
        valid(
            r#"
name = "test"
version = 1
"#,
            str![[r#"

name = "test"
version = 1

"#]],
        );
    }

    #[test]
    fn datetime_with_t_unchanged() {
        valid(
            r#"
created = 2025-12-26T10:30:00
"#,
            str![[r#"

created = 2025-12-26T10:30:00

"#]],
        );
    }

    #[test]
    fn datetime_with_space_normalized() {
        valid(
            r#"
created = 2025-12-26 10:30:00
"#,
            str![[r#"

created = 2025-12-26T10:30:00

"#]],
        );
    }

    #[test]
    fn local_date_only_unchanged() {
        valid(
            r#"
date = 2025-12-26
"#,
            str![[r#"

date = 2025-12-26

"#]],
        );
    }

    #[test]
    fn local_time_only_unchanged() {
        valid(
            r#"
time = 10:30:00
"#,
            str![[r#"

time = 10:30:00

"#]],
        );
    }

    #[test]
    fn offset_datetime_with_space_normalized() {
        valid(
            r#"
utc = 2025-12-26 10:30:00Z
"#,
            str![[r#"

utc = 2025-12-26T10:30:00Z

"#]],
        );
    }

    #[test]
    fn offset_datetime_with_timezone_normalized() {
        valid(
            r#"
eastern = 2025-12-26 10:30:00-05:00
"#,
            str![[r#"

eastern = 2025-12-26T10:30:00-05:00

"#]],
        );
    }

    #[test]
    fn datetime_in_array() {
        valid(
            r#"
dates = [
    2025-12-26 10:30:00,
    2025-12-27 11:00:00,
]
"#,
            str![[r#"

dates = [
    2025-12-26T10:30:00,
    2025-12-27T11:00:00,
]

"#]],
        );
    }

    #[test]
    fn datetime_in_inline_table() {
        valid(
            r#"
event = { start = 2025-12-26 10:30:00, end = 2025-12-26 12:00:00 }
"#,
            str![[r#"

event = { start = 2025-12-26T10:30:00, end = 2025-12-26T12:00:00 }

"#]],
        );
    }

    #[test]
    fn mixed_datetime_formats() {
        valid(
            r#"
with_t = 2025-12-26T10:30:00
with_space = 2025-12-26 10:30:00
date_only = 2025-12-26
time_only = 10:30:00
"#,
            str![[r#"

with_t = 2025-12-26T10:30:00
with_space = 2025-12-26T10:30:00
date_only = 2025-12-26
time_only = 10:30:00

"#]],
        );
    }

    #[test]
    fn datetime_with_fractional_seconds() {
        valid(
            r#"
precise = 2025-12-26 10:30:00.123456
"#,
            str![[r#"

precise = 2025-12-26T10:30:00.123456

"#]],
        );
    }

    #[test]
    fn datetime_with_lowercase_t_normalized() {
        // RFC 3339 allows lowercase 't' as separator
        // We normalize to uppercase 'T' for consistency
        valid(
            r#"
created = 2025-12-26t10:30:00
"#,
            str![[r#"

created = 2025-12-26T10:30:00

"#]],
        );
    }

    #[test]
    fn offset_datetime_with_positive_timezone() {
        valid(
            r#"
tokyo = 2025-12-26 10:30:00+09:00
"#,
            str![[r#"

tokyo = 2025-12-26T10:30:00+09:00

"#]],
        );
    }
}
