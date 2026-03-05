use std::borrow::Cow;

use crate::toml::{TokenIndices, TokenKind, TomlToken};

/// Wraps standalone TOML comment lines that exceed `comment_width`.
///
/// Inline comments are left untouched.
///
/// Assumptions:
/// - newlines normalized
/// - trailing spaces trimmed
#[tracing::instrument]
pub fn wrap_comment_lines<'i>(
    tokens: &mut crate::toml::TomlTokens<'i>,
    wrap: bool,
    comment_width: usize,
) {
    if !wrap || comment_width == 0 {
        return;
    }

    let mut col: usize = 0;
    let mut at_line_start = true;
    let mut indices = TokenIndices::new();
    while let Some(i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::Newline => {
                col = 0;
                at_line_start = true;
            }
            TokenKind::Whitespace => {
                col += tokens.tokens[i].raw.len();
            }
            TokenKind::Comment if at_line_start => {
                if col + tokens.tokens[i].raw.len() <= comment_width {
                    continue;
                }

                let (prefix, text) = {
                    let raw = &*tokens.tokens[i].raw;
                    let (p, t) = split_comment(raw);
                    (p.to_owned(), t.to_owned())
                };

                let prefix_col = col + prefix.len();
                if prefix_col >= comment_width {
                    continue;
                }
                let available = comment_width - prefix_col;
                let wrapped = word_wrap(&text, available);

                if wrapped.len() <= 1 {
                    continue;
                }

                let followed_by_nl = tokens
                    .tokens
                    .get(i + 1)
                    .map_or(false, |t| t.kind == TokenKind::Newline);
                if !followed_by_nl {
                    continue;
                }

                let indent = if i > 0 && tokens.tokens[i - 1].kind == TokenKind::Whitespace {
                    tokens.tokens[i - 1].raw.as_ref().to_owned()
                } else {
                    String::new()
                };

                tokens.tokens[i].raw = Cow::Owned(format!("{prefix}{}", wrapped[0]));

                let mut new_tokens: Vec<TomlToken<'i>> = Vec::new();
                for line in &wrapped[1..] {
                    if !indent.is_empty() {
                        new_tokens.push(TomlToken {
                            kind: TokenKind::Whitespace,
                            encoding: None,
                            decoded: None,
                            scalar: None,
                            raw: Cow::Owned(indent.clone()),
                        });
                    }
                    new_tokens.push(TomlToken {
                        kind: TokenKind::Comment,
                        encoding: None,
                        decoded: None,
                        scalar: None,
                        raw: Cow::Owned(format!("{prefix}{line}")),
                    });
                    new_tokens.push(TomlToken::NL);
                }

                let n = new_tokens.len();
                debug_assert_eq!(tokens.tokens[i].kind, TokenKind::Comment);
                debug_assert_eq!(tokens.tokens[i + 1].kind, TokenKind::Newline);
                tokens.tokens.splice(i + 2..i + 2, new_tokens);
                indices.set_next_index(i + 2 + n);
                col = 0;
                at_line_start = true;
            }
            _ => {
                col += tokens.tokens[i].raw.len();
                at_line_start = false;
            }
        }
    }
}

/// Returns `(prefix, text)` where prefix is the leading `#`s plus an optional space.
fn split_comment(comment: &str) -> (&str, &str) {
    let hash_end = comment
        .char_indices()
        .find(|(_, c)| *c != '#')
        .map(|(i, _)| i)
        .unwrap_or(comment.len());

    let prefix_end = if comment[hash_end..].starts_with(' ') {
        hash_end + 1
    } else {
        hash_end
    };

    (&comment[..prefix_end], &comment[prefix_end..])
}

/// Greedy word-wrap; words longer than `max_width` are kept whole on their own line.
fn word_wrap(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_owned()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }

    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }

    lines
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
