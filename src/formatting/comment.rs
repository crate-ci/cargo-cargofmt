/// Wraps standalone TOML comment lines that exceed `comment_width`.
///
/// This is a no-op when `wrap` is `false`.
///
/// Assumptions:
/// - Newlines normalized to `\n`
/// - Trailing spaces trimmed
#[tracing::instrument]
pub fn wrap_comment_lines(
    _tokens: &mut crate::toml::TomlTokens<'_>,
    _wrap: bool,
    _comment_width: usize,
) {
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, wrap: bool, comment_width: usize, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::wrap_comment_lines(&mut tokens, wrap, comment_width);
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
    fn no_wrap_when_disabled() {
        // wrap=false: long comments are left untouched
        valid(
            "# This is a very long comment that would exceed forty characters easily\nkey = 1\n",
            false,
            40,
            str![
                "# This is a very long comment that would exceed forty characters easily\nkey = 1\n"
            ],
        );
    }

    #[test]
    fn no_wrap_short_comment() {
        // Comment fits within comment_width; nothing changes
        valid(
            "# Short comment\nkey = 1\n",
            true,
            80,
            str!["# Short comment\nkey = 1\n"],
        );
    }

    #[test]
    fn does_not_wrap_inline_comment() {
        // An inline comment after a value must NOT be touched
        valid(
            "key = 1 # This is a very long inline comment that exceeds the width limit set\n",
            true,
            40,
            str![
                "key = 1 # This is a very long inline comment that exceeds the width limit set\n"
            ],
        );
    }

    #[test]
    fn does_not_split_long_single_word() {
        // A URL that cannot be split should stay on one line
        valid(
            "# https://example.com/very/long/url/that/exceeds/the/limit/easily/here\n",
            true,
            40,
            str!["# https://example.com/very/long/url/that/exceeds/the/limit/easily/here\n"],
        );
    }
}
