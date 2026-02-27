use std::borrow::Cow;

use crate::toml::{TokenKind, TomlToken};

/// Wraps standalone TOML comment lines that exceed `comment_width`.
///
/// This is a no-op when `wrap` is `false`.
///
/// Assumptions:
/// - Newlines normalized to `\n`
/// - Trailing spaces trimmed
#[tracing::instrument]
pub fn wrap_comment_lines<'i>(
    tokens: &mut crate::toml::TomlTokens<'i>,
    wrap: bool,
    comment_width: usize,
) {
    if !wrap || comment_width == 0 {
        return;
    }

    let mut i = 0;
    while i < tokens.len() {
        if tokens.tokens[i].kind != TokenKind::Comment {
            i += 1;
            continue;
        }

        // Only process standalone comments (not inline after a value)
        if !is_standalone_comment(tokens, i) {
            i += 1;
            continue;
        }

        let col = line_column(tokens, i);
        let comment_raw = tokens.tokens[i].raw.to_string();
        let total_width = col + comment_raw.len();

        // Already fits within the limit
        if total_width <= comment_width {
            i += 1;
            continue;
        }

        // Split the comment into its prefix (e.g., "# ") and wrappable text
        let (prefix_str, text_str) = split_comment(&comment_raw);
        let prefix = prefix_str.to_owned();
        let text = text_str.to_owned();

        // Available width for the text portion of each wrapped line
        let prefix_col = col + prefix.len();
        if prefix_col >= comment_width {
            // No room to wrap even one character — skip
            i += 1;
            continue;
        }
        let available = comment_width - prefix_col;

        let wrapped = word_wrap(&text, available);
        if wrapped.len() <= 1 {
            // Either a single unsplittable word (e.g. URL), or already fits
            i += 1;
            continue;
        }

        // Require a newline after this comment so we have a safe insertion point
        let has_newline_after =
            i + 1 < tokens.len() && tokens.tokens[i + 1].kind == TokenKind::Newline;
        if !has_newline_after {
            i += 1;
            continue;
        }

        // Capture the indentation for continuation lines before mutating tokens
        let indent = line_indent(tokens, i);

        // Update the comment token with the first wrapped line
        tokens.tokens[i].raw = Cow::Owned(format!("{prefix}{}", wrapped[0]));

        // Build additional tokens to insert after the original terminating newline (at i+2)
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

        let extra_count = new_tokens.len();
        // Insert after comment (i) and its original terminating newline (i+1)
        let insert_at = i + 2;
        tokens.tokens.splice(insert_at..insert_at, new_tokens);

        // Advance past: comment(1) + original-newline(1) + inserted-tokens(extra_count)
        i += 2 + extra_count;
    }
}

/// Returns `true` if the comment at `comment_i` is standalone (not inline after a value).
fn is_standalone_comment(tokens: &crate::toml::TomlTokens<'_>, comment_i: usize) -> bool {
    for j in (0..comment_i).rev() {
        match tokens.tokens[j].kind {
            TokenKind::Newline => return true,
            TokenKind::Whitespace => continue,
            _ => return false,
        }
    }
    true
}

/// Returns the column position (characters since the last newline) of token `i`.
fn line_column(tokens: &crate::toml::TomlTokens<'_>, i: usize) -> usize {
    let mut col = 0;
    for j in (0..i).rev() {
        if tokens.tokens[j].kind == TokenKind::Newline {
            break;
        }
        col += tokens.tokens[j].raw.len();
    }
    col
}

/// Returns the leading whitespace string for the line containing `comment_i`.
fn line_indent(tokens: &crate::toml::TomlTokens<'_>, comment_i: usize) -> String {
    if comment_i == 0 {
        return String::new();
    }
    let prev = comment_i - 1;
    if tokens.tokens[prev].kind == TokenKind::Whitespace {
        let at_line_start =
            prev == 0 || tokens.tokens[prev - 1].kind == TokenKind::Newline;
        if at_line_start {
            return tokens.tokens[prev].raw.to_string();
        }
    }
    String::new()
}

/// Splits a TOML comment string into its prefix (e.g. `"# "`) and the text content.
///
/// The prefix consists of all leading `#` characters plus at most one trailing space.
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

/// Word-wraps `text` into lines of at most `max_width` characters, splitting at whitespace.
///
/// Words longer than `max_width` are placed on their own line without splitting.
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
        valid(
            "# Short comment\nkey = 1\n",
            true,
            80,
            str!["# Short comment\nkey = 1\n"],
        );
    }

    #[test]
    fn does_not_wrap_inline_comment() {
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
        valid(
            "# https://example.com/very/long/url/that/exceeds/the/limit/easily/here\n",
            true,
            40,
            str!["# https://example.com/very/long/url/that/exceeds/the/limit/easily/here\n"],
        );
    }

    #[test]
    fn wraps_long_standalone_comment() {
        // available = 40 - 2 ("# ") = 38
        // "This comment is too long and needs to" (37) fits; adding "be" = 40 > 38 → break
        valid(
            "# This comment is too long and needs to be wrapped at the right boundary here\nkey = 1\n",
            true,
            40,
            str![[r#"
# This comment is too long and needs to
# be wrapped at the right boundary here
key = 1
"#]],
        );
    }

    #[test]
    fn wraps_into_three_lines() {
        // available = 30 - 2 = 28
        valid(
            "# one two three four five six seven eight nine ten eleven twelve thirteen\nkey = 1\n",
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
    fn wraps_indented_comment() {
        // col=2, prefix_col=4, available=36
        valid(
            "  # This is an indented comment that is way too long and needs to be wrapped here\n",
            true,
            40,
            str![[r#"
  # This is an indented comment that is
  # way too long and needs to be wrapped
  # here
"#]],
        );
    }

    #[test]
    fn wrap_preserves_subsequent_entries() {
        // available = 40 - 2 = 38
        // "A long comment that should be wrapped" (37) fits; "because" pushes → break
        valid(
            "# A long comment that should be wrapped because it exceeds the limit\nkey = \"value\"\nother = 1\n",
            true,
            40,
            str![[r#"
# A long comment that should be wrapped
# because it exceeds the limit
key = "value"
other = 1
"#]],
        );
    }

    #[test]
    fn wrap_double_hash_prefix() {
        // prefix = "## " (3 chars), available = 37
        valid(
            "## A long comment with double hash that exceeds forty characters total here\n",
            true,
            40,
            str![[r#"
## A long comment with double hash that
## exceeds forty characters total here
"#]],
        );
    }

    #[test]
    fn wrap_multiple_comments() {
        valid(
            "# First long comment that exceeds the limit at forty chars set here now\n# Second long comment that also exceeds the limit at forty chars set\nkey = 1\n",
            true,
            40,
            str![[r#"
# First long comment that exceeds the
# limit at forty chars set here now
# Second long comment that also exceeds
# the limit at forty chars set
key = 1
"#]],
        );
    }
}
