/// Wraps standalone TOML comment lines that exceed `comment_width`.
///
/// Inline comments are left untouched.
///
/// Assumptions:
/// - newlines normalized
/// - trailing spaces trimmed
#[tracing::instrument]
pub fn wrap_comment_lines<'i>(
    _tokens: &mut crate::toml::TomlTokens<'i>,
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
        valid(
            "
# This package requires nightly because of unstable async trait features
key = 1
",
            false,
            40,
            str![[r#"

# This package requires nightly because of unstable async trait features
key = 1

"#]],
        );
    }

    #[test]
    fn no_wrap_short_comment() {
        valid(
            "
# Short comment
key = 1
",
            true,
            80,
            str![[r#"

# Short comment
key = 1

"#]],
        );
    }

    #[test]
    fn does_not_wrap_inline_comment() {
        valid(
            "
key = 1 # see rustfmt's comment_width setting for the equivalent option in Rust
",
            true,
            40,
            str![[r#"

key = 1 # see rustfmt's comment_width setting for the equivalent option in Rust

"#]],
        );
    }

    #[test]
    fn wraps_comment_containing_url() {
        valid(
            "
# See https://doc.rust-lang.org/cargo/reference/config.html for more details
key = 1
",
            true,
            40,
            str![[r#"

# See
# https://doc.rust-lang.org/cargo/reference/config.html
# for more details
key = 1

"#]],
        );
    }

    #[test]
    fn wraps_long_standalone_comment() {
        valid(
            "
# Pinned to avoid breaking changes during the 2024 edition migration
key = 1
",
            true,
            40,
            str![[r#"

# Pinned to avoid breaking changes
# during the 2024 edition migration
key = 1

"#]],
        );
    }

    #[test]
    fn wraps_into_multiple_lines() {
        valid(
            "
# one two three four five six seven eight nine ten eleven twelve thirteen
key = 1
",
            true,
            30,
            str![[r#"

# one two three four five six
# seven eight nine ten eleven
# twelve thirteen
key = 1

"#]],
        );
    }

    #[test]
    fn wraps_comment_preserving_indent() {
        // leading whitespace carries over to continuation lines
        valid(
            "
[dependencies]
  # This dependency provides async runtime support and is required for all async operations
  tokio = { version = \"1\", features = [\"full\"] }
",
            true,
            60,
            str![[r#"

[dependencies]
  # This dependency provides async runtime support and is
  # required for all async operations
  tokio = { version = "1", features = ["full"] }

"#]],
        );
    }

    #[test]
    fn wrap_preserves_subsequent_entries() {
        valid(
            "
# Override max_width for this workspace to match the project style guide
key = \"value\"
other = 1
",
            true,
            40,
            str![[r#"

# Override max_width for this workspace
# to match the project style guide
key = "value"
other = 1

"#]],
        );
    }

    #[test]
    fn wraps_comment_with_multi_hash_prefix() {
        // ## prefix should carry over to continuation lines
        valid(
            "
## All packages in this workspace share the same maximum column width
",
            true,
            40,
            str![[r#"

## All packages in this workspace share
## the same maximum column width

"#]],
        );
    }

    #[test]
    fn wrap_multiple_comments() {
        valid(
            "
# Max line length used when reformatting comment and string tokens
# Set to false to disable wrapping and leave all lines unchanged
key = 1
",
            true,
            40,
            str![[r#"

# Max line length used when reformatting
# comment and string tokens
# Set to false to disable wrapping and
# leave all lines unchanged
key = 1

"#]],
        );
    }
}
