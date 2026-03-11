use unicode_width::UnicodeWidthChar;

use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

/// Detects line overflow errors
///
/// After formatting, lines that still exceed the maximum width are reported.
/// Standalone comments, inline comments (where the comment itself causes the overflow),
/// and string literals are exempt from these errors
pub fn check_line_overflow(
    tokens: &TomlTokens<'_>,
    max_width: usize,
    tab_spaces: usize,
) -> Vec<(usize, usize)> {
    let mut overflows = Vec::new();
    let mut line_no = 1;
    let mut line_start = 0;

    for i in tokens.indices() {
        let is_newline = tokens.tokens[i].kind == TokenKind::Newline;
        let is_eof = i == tokens.len() - 1; // For last line (having no trailing newline)

        let end_idx = if is_newline {
            Some(i)
        } else if is_eof {
            Some(i + 1)
        } else {
            None
        };

        if let Some(end_idx) = end_idx {
            if line_start < end_idx {
                let line_tokens = &tokens.tokens[line_start..end_idx];
                let line_width = line_display_width(line_tokens, tab_spaces);
                if line_width > max_width && !is_exempt(line_tokens, max_width, tab_spaces) {
                    overflows.push((line_no, line_width));
                }
            }

            if is_newline {
                line_no += 1;
                line_start = i + 1;
            }
        }
    }

    overflows
}

fn line_display_width(line_tokens: &[TomlToken<'_>], tab_spaces: usize) -> usize {
    line_tokens
        .iter()
        .map(|t| token_display_width(&t.raw, tab_spaces))
        .sum()
}

fn token_display_width(raw: &str, tab_spaces: usize) -> usize {
    raw.chars()
        .map(|c| {
            if c == '\t' {
                tab_spaces
            } else {
                c.width().unwrap_or(0)
            }
        })
        .sum()
}

fn is_exempt(line_tokens: &[TomlToken<'_>], max_width: usize, tab_spaces: usize) -> bool {
    // Case 1: Standalone comment
    let first_meaningful = line_tokens.iter().find(|t| t.kind != TokenKind::Whitespace);
    if matches!(first_meaningful, Some(t) if t.kind == TokenKind::Comment) {
        return true;
    }

    // Case 2: Inline comment — find # that's NOT inside a quoted string
    let comment_pos = line_tokens
        .iter()
        .position(|t| t.kind == TokenKind::Comment);

    if let Some(pos) = comment_pos {
        let code_tokens = &line_tokens[..pos];
        let code_width: usize = code_tokens
            .iter()
            .map(|t| token_display_width(t.raw.trim_end(), tab_spaces))
            .sum();
        if code_width <= max_width {
            return true; // overflow caused by comment, not code
        }
    }

    // Case 3: String literal
    let scalar_pos = line_tokens
        .iter()
        .position(|t| t.kind == TokenKind::Scalar && t.scalar.is_none());

    if let Some(pos) = scalar_pos {
        let prefix_width: usize = line_tokens[..pos]
            .iter()
            .map(|t| token_display_width(t.raw.trim_end(), tab_spaces))
            .sum();
        if prefix_width <= max_width {
            return true; // overflow caused by string value, not the prefix
        }
    }

    false
}

#[cfg(test)]
mod test {
    use crate::toml::TomlTokens;

    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, max_width: usize, tab_spaces: usize, expected: impl IntoData) {
        let tokens = TomlTokens::parse(input);
        let actual = super::check_line_overflow(&tokens, max_width, tab_spaces);
        let actual_str = format!("{:?}", actual);
        assert_data_eq!(&actual_str, expected);
    }

    #[test]
    fn no_overflow() {
        let input = "[package]\nname = \"app\"\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_table_header() {
        let input = "[very-long-table-header-name]\n";
        valid(input, 20, 4, str![[r#"[(1, 29)]"#]]);
    }

    #[test]
    fn exempt_standalone_comment() {
        let input = "# A very long comment line\n";
        valid(input, 8, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn exempt_inline_comment() {
        let input = "key = \"value\"  # very long comment here\n";
        valid(input, 10, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_inline_comment() {
        let input = "very-long-key-name = 1 # short comment\n";
        valid(input, 15, 4, str![[r#"[(1, 38)]"#]]);
    }

    #[test]
    fn overflow_from_long_key() {
        let input = "a_very_long_key_cause_overflow = \"b\"\n";
        valid(input, 10, 4, str![[r#"[(1, 36)]"#]]);
    }

    #[test]
    fn exempt_string_literal() {
        let input = "description = \"https://docs.rs/test-overflow-with-super-long-paths-and-extra-sections-that-keep-going-forever\"\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn long_key_with_string_literal_errors() {
        let input = "very_long_key_name_here = \"short\"\n";
        valid(input, 20, 4, str![[r#"[(1, 33)]"#]]);
    }
}
