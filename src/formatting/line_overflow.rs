use unicode_width::UnicodeWidthChar;

pub fn check_line_overflow(formatted: &str, max_width: usize) -> Vec<(usize, usize)> {
    let mut overflows = Vec::new();
    for (line_no, each_line) in formatted.lines().enumerate() {
        let line_len = token_width(each_line, 4);
        if line_len > max_width && !is_exempt(each_line, max_width) {
            overflows.push((line_no + 1, line_len));
        }
    }
    overflows
}

/// Check if a line should be exempt from overflow error.
fn is_exempt(line: &str, max_width: usize) -> bool {
    let trimmed = line.trim_start();

    // Case 1: Standalone comment
    if trimmed.starts_with('#') {
        return true;
    }

    // Case 2: Inline comment — find # that's NOT inside a quoted string
    let uncommented_part = strip_inline_comment(line);
    if uncommented_part.len() < line.len() {
        // There was a comment — check if code part alone fits
        let its_width = token_width(uncommented_part.trim_end(), 4);
        if its_width <= max_width {
            return true; // overflow caused by comment, not code
        }
    }

    //Case 3 : string literal
    let key_part = strip_string_value(line);
    if key_part.len() < line.len() {
        let its_width = token_width(key_part.trim_end(), 4);
        if its_width <= max_width {
            return true;
        }
    }
    false
}

/// Strip inline comment from a line, respecting quoted strings.
/// Returns the code portion before the comment.
fn strip_inline_comment(line: &str) -> &str {
    let mut in_quotes = false;
    for (idx, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            '#' if !in_quotes => return &line[..idx],
            _ => {}
        }
    }

    line
}

fn strip_string_value(line: &str) -> &str {
    let mut equal_appears = false;
    for (idx, c) in line.char_indices() {
        match c {
            '=' => equal_appears = true,
            '"' if equal_appears => return &line[..idx],
            _ => {}
        }
    }

    line
}

/// Calculate display width of a token.
fn token_width(raw: &str, tab_spaces: usize) -> usize {
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

#[cfg(test)]
mod test {

    #[track_caller]
    fn valid(input: &str, max_width: usize, expected: Vec<(usize, usize)>) {
        let actual = super::check_line_overflow(input, max_width);
        assert_eq!(actual, expected);
    }

    #[test]
    fn no_overflow() {
        let input = "[package]\nname = \"app\"\n";
        valid(input, 20, vec![]);
    }

    #[test]
    fn overflow_table_header() {
        let input = "[very-long-table-header-name]\n";
        valid(input, 20, vec![(1, 29)]);
    }

    #[test]
    fn exempt_standalone_comment() {
        let input = "# This is a very long comment line\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn exempt_inline_comment() {
        let input = "a = \"b\"  # very long comment here\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn overflow_from_long_key() {
        let input = "a_very_long_key = \"b\"  # very long comment here";
        valid(input, 10, vec![(1, 47)]);
    }

    #[test]
    fn exempt_string_literal() {
        let input = "desc = \"A very long description\"\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn not_exempt_hash_inside_string() {
        let input = "long-key-name-here = \"rust\"\n";
        valid(input, 10, vec![(1, 27)]);
    }
}
