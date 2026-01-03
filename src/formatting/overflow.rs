use std::borrow::Cow;

use crate::toml::TokenIndices;
use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

/// Reflow arrays that exceed `max_width` to vertical layout.
///
/// Uses incremental depth tracking for O(n) complexity instead of
/// rescanning from the start for each array.
#[tracing::instrument]
pub fn reflow_arrays(tokens: &mut TomlTokens<'_>, max_width: usize, tab_spaces: usize) {
    let mut indices = TokenIndices::new();
    let mut inline_table_depth = 0usize;
    let mut nesting_depth = 0usize;

    while let Some(i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::InlineTableOpen => {
                inline_table_depth += 1;
                nesting_depth += 1;
            }
            TokenKind::InlineTableClose => {
                inline_table_depth = inline_table_depth.saturating_sub(1);
                nesting_depth = nesting_depth.saturating_sub(1);
            }
            TokenKind::ArrayOpen => {
                if let Some(close) =
                    reflow_target(tokens, i, inline_table_depth, max_width, tab_spaces)
                {
                    reflow_array_to_vertical(tokens, i, close, tab_spaces, nesting_depth);
                }
                nesting_depth += 1;
            }
            TokenKind::ArrayClose => {
                nesting_depth = nesting_depth.saturating_sub(1);
            }
            _ => {}
        }
    }
}

/// Determine if array at `open_index` should be reflowed.
///
/// Returns `Some(close_index)` if reflow should happen, `None` otherwise.
/// Pure decision function - checks inline table depth, finds matching bracket,
/// and evaluates width constraints.
fn reflow_target(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    inline_table_depth: usize,
    max_width: usize,
    tab_spaces: usize,
) -> Option<usize> {
    // Skip arrays inside inline tables
    if inline_table_depth > 0 {
        return None;
    }
    let close_index = find_array_close(tokens, open_index)?;
    if !should_reflow_array(tokens, open_index, close_index, max_width, tab_spaces) {
        return None;
    }
    Some(close_index)
}

/// Check if an array should be reflowed to vertical layout.
///
/// Note: Inline table check is performed by the caller for O(n) efficiency.
fn should_reflow_array(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    max_width: usize,
    tab_spaces: usize,
) -> bool {
    // Already vertical? Don't reflow
    if is_array_vertical(tokens, open_index, close_index) {
        return false;
    }

    // Calculate line width including the array
    let line_start = find_line_start(tokens, open_index);
    let line_width: usize = tokens.tokens[line_start..=close_index]
        .iter()
        .map(|t| token_width(&t.raw, tab_spaces))
        .sum();

    line_width > max_width
}

/// Check if array already has vertical layout (contains newlines).
fn is_array_vertical(tokens: &TomlTokens<'_>, open_index: usize, close_index: usize) -> bool {
    tokens.tokens[open_index..=close_index]
        .iter()
        .any(|t| t.kind == TokenKind::Newline)
}

/// Find the matching `ArrayClose` for an `ArrayOpen`.
///
/// Returns `None` if no matching close bracket is found (malformed input).
fn find_array_close(tokens: &TomlTokens<'_>, open_index: usize) -> Option<usize> {
    let mut depth = 0;
    for i in open_index..tokens.len() {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen => depth += 1,
            TokenKind::ArrayClose => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the start of the current line (index after last newline).
fn find_line_start(tokens: &TomlTokens<'_>, from_index: usize) -> usize {
    for i in (0..from_index).rev() {
        if tokens.tokens[i].kind == TokenKind::Newline {
            return i + 1;
        }
    }
    0
}

/// Calculate display width of a token.
fn token_width(raw: &str, tab_spaces: usize) -> usize {
    raw.chars()
        .map(|c| if c == '\t' { tab_spaces } else { 1 })
        .sum()
}

/// Convert a horizontal array to vertical layout.
///
/// `nesting_depth` is the current nesting level (arrays + inline tables) before
/// this array, tracked incrementally by the caller for O(n) efficiency.
fn reflow_array_to_vertical(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    tab_spaces: usize,
    nesting_depth: usize,
) {
    let indent = make_indent(nesting_depth + 1, tab_spaces);
    let close_indent = make_indent(nesting_depth, tab_spaces);

    // Collect positions where we need to insert newline + indent
    // Each entry is (index, indent_string)
    // We'll work backwards to avoid index shifting issues
    let mut insertions: Vec<(usize, String)> = Vec::new();

    // After the opening bracket
    insertions.push((open_index + 1, indent.clone()));

    // After each comma (ValueSep) at this array's depth only
    let mut local_depth = 0;
    for i in (open_index + 1)..close_index {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => local_depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => local_depth -= 1,
            TokenKind::ValueSep if local_depth == 0 => {
                // Skip trailing comma (nothing after it except whitespace/close)
                if is_trailing_comma(tokens, i, close_index) {
                    continue;
                }
                // Remove any existing whitespace after comma
                if i + 1 < close_index && tokens.tokens[i + 1].kind == TokenKind::Whitespace {
                    tokens.tokens[i + 1] = TomlToken::EMPTY;
                }
                insertions.push((i + 1, indent.clone()));
            }
            _ => {}
        }
    }

    // Before the closing bracket
    insertions.push((close_index, close_indent));

    // Apply insertions in reverse order to maintain correct indices
    // Insert Whitespace (indent) first, then Newline, so they end up as Newline then Whitespace
    for (index, indent_str) in insertions.into_iter().rev() {
        // Insert indent (if any)
        if !indent_str.is_empty() {
            let indent_token = TomlToken {
                kind: TokenKind::Whitespace,
                encoding: None,
                decoded: None,
                scalar: None,
                raw: Cow::Owned(indent_str),
            };
            tokens.tokens.insert(index, indent_token);
        }
        // Insert newline
        tokens.tokens.insert(index, TomlToken::NL);
    }

    tokens.trim_empty_whitespace();
}

/// Check if a comma is a trailing comma (only whitespace between it and close bracket).
fn is_trailing_comma(tokens: &TomlTokens<'_>, comma_index: usize, close_index: usize) -> bool {
    tokens.tokens[(comma_index + 1)..close_index]
        .iter()
        .all(|t| t.kind == TokenKind::Whitespace)
}

fn make_indent(depth: usize, tab_spaces: usize) -> String {
    " ".repeat(depth * tab_spaces)
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    use crate::toml::TomlTokens;

    const DEFAULT_TAB_SPACES: usize = 4;

    #[track_caller]
    fn valid(input: &str, max_width: usize, expected: impl IntoData) {
        let mut tokens = TomlTokens::parse(input);
        super::reflow_arrays(&mut tokens, max_width, DEFAULT_TAB_SPACES);
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
    fn short_array_not_reflowed() {
        // Short arrays stay horizontal
        valid(
            r#"deps = ["a", "b"]
"#,
            80,
            str![[r#"
deps = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn long_array_reflowed() {
        valid(
            r#"deps = ["foo", "bar", "baz"]
"#,
            20,
            str![[r#"
deps = [
    "foo",
    "bar",
    "baz"
]

"#]],
        );
    }

    #[test]
    fn already_vertical_not_modified() {
        // Already vertical arrays stay unchanged
        valid(
            r#"deps = [
    "foo",
    "bar",
]
"#,
            20,
            str![[r#"
deps = [
    "foo",
    "bar",
]

"#]],
        );
    }

    #[test]
    fn nested_array_reflowed() {
        // Inner arrays fit on their lines (14 chars < 20), so stay horizontal
        valid(
            r#"matrix = [[1, 2, 3], [4, 5, 6]]
"#,
            20,
            str![[r#"
matrix = [
    [1, 2, 3],
    [4, 5, 6]
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_array() {
        // At max_width=5, all levels exceed after parent reflows
        valid(
            r#"x = [[[1]]]
"#,
            5,
            str![[r#"
x = [
    [
        [
            1
        ]
    ]
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_partial_reflow() {
        // At max_width=10, outer reflows but "    [[1]]" = 9 chars < 10, so inner stays horizontal
        valid(
            r#"x = [[[1]]]
"#,
            10,
            str![[r#"
x = [
    [[1]]
]

"#]],
        );
    }

    #[test]
    fn array_with_inline_table() {
        valid(
            r#"deps = [{name = "foo"}, {name = "bar"}]
"#,
            30,
            str![[r#"
deps = [
    {name = "foo"},
    {name = "bar"}
]

"#]],
        );
    }

    #[test]
    fn empty_array_not_reflowed() {
        // Empty arrays stay unchanged
        valid(
            r#"deps = []
"#,
            10,
            str![[r#"
deps = []

"#]],
        );
    }

    #[test]
    fn array_at_exact_max_width() {
        // At exact max_width, no reflow needed
        valid(
            r#"a = [1, 2]
"#,
            10,
            str![[r#"
a = [1, 2]

"#]],
        );
    }

    #[test]
    fn array_one_over_max_width() {
        valid(
            r#"a = [1, 2]
"#,
            9,
            str![[r#"
a = [
    1,
    2
]

"#]],
        );
    }

    #[test]
    fn max_width_zero_reflows_everything() {
        valid(
            r#"a = [1]
"#,
            0,
            str![[r#"
a = [
    1
]

"#]],
        );
    }

    #[test]
    fn max_width_max_reflows_nothing() {
        // Very large max_width means no reflow
        valid(
            r#"deps = ["foo", "bar", "baz", "qux", "quux"]
"#,
            usize::MAX,
            str![[r#"
deps = ["foo", "bar", "baz", "qux", "quux"]

"#]],
        );
    }

    #[test]
    fn long_inline_table_not_reflowed() {
        // Inline tables stay on single line even when exceeding max_width
        valid(
            r#"deps = [{name = "very-long-name", version = "1.0.0", features = ["a", "b"]}]
"#,
            40,
            str![[r#"
deps = [
    {name = "very-long-name", version = "1.0.0", features = ["a", "b"]}
]

"#]],
        );
    }

    #[test]
    fn inline_table_containing_array() {
        // Array inside inline table should not be reflowed
        valid(
            r#"dep = [{features = ["a", "b", "c"]}]
"#,
            20,
            str![[r#"
dep = [
    {features = ["a", "b", "c"]}
]

"#]],
        );
    }

    #[test]
    fn nested_inline_tables() {
        // Nested inline tables should not be reflowed
        valid(
            r#"items = [{outer = {inner = "value"}}]
"#,
            20,
            str![[r#"
items = [
    {outer = {inner = "value"}}
]

"#]],
        );
    }

    #[test]
    fn array_with_comments() {
        valid(
            r#"deps = ["foo", "bar"] # comment
"#,
            20,
            str![[r#"
deps = [
    "foo",
    "bar"
] # comment

"#]],
        );
    }

    #[test]
    fn array_with_trailing_comma() {
        valid(
            r#"deps = ["foo", "bar",]
"#,
            15,
            str![[r#"
deps = [
    "foo",
    "bar",
]

"#]],
        );
    }

    #[test]
    fn very_long_single_element() {
        // Single element exceeding max_width still reflows
        valid(
            r#"deps = ["this-is-a-very-long-package-name"]
"#,
            20,
            str![[r#"
deps = [
    "this-is-a-very-long-package-name"
]

"#]],
        );
    }

    #[test]
    fn array_in_table_section() {
        valid(
            r#"[package]
keywords = ["cli", "toml", "formatter"]
"#,
            30,
            str![[r#"
[package]
keywords = [
    "cli",
    "toml",
    "formatter"
]

"#]],
        );
    }

    #[test]
    fn unicode_values_in_array() {
        valid(
            r#"names = ["日本語", "中文", "한국어"]
"#,
            20,
            str![[r#"
names = [
    "日本語",
    "中文",
    "한국어"
]

"#]],
        );
    }

    #[test]
    fn multiline_string_in_array() {
        // Newlines inside string literals don't count as array being vertical
        valid(
            r#"items = ["""
multi
line
"""]
"#,
            10,
            str![[r#"
items = [
    """
multi
line
"""
]

"#]],
        );
    }

    #[test]
    fn unclosed_array_not_panics() {
        // Malformed input: unclosed array bracket
        // Should gracefully skip rather than panic
        // Note: not using valid() since output is invalid TOML
        let input = r#"deps = ["foo", "bar"
"#;
        let mut tokens = TomlTokens::parse(input);
        super::reflow_arrays(&mut tokens, 10, DEFAULT_TAB_SPACES);
        let actual = tokens.to_string();
        assert_data_eq!(
            &actual,
            str![[r#"
deps = ["foo", "bar"

"#]]
        );
    }

    #[test]
    fn dotted_key_width_included() {
        // Width calculation should include dotted key
        // "foo.bar.baz = [\"a\", \"b\"]" = 24 chars
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            23,
            str![[r#"
foo.bar.baz = [
    "a",
    "b"
]

"#]],
        );
    }

    #[test]
    fn dotted_key_at_exact_width() {
        // Exactly at limit stays horizontal
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            24,
            str![[r#"
foo.bar.baz = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn quoted_key() {
        valid(
            r#""my.key" = ["x", "y"]
"#,
            15,
            str![[r#"
"my.key" = [
    "x",
    "y"
]

"#]],
        );
    }

    #[test]
    fn literal_strings() {
        // Single-quoted literal strings
        valid(
            r#"paths = ['foo', 'bar']
"#,
            15,
            str![[r#"
paths = [
    'foo',
    'bar'
]

"#]],
        );
    }

    #[test]
    fn mixed_types_in_array() {
        valid(
            r#"mixed = [1, "two", true, 3.14]
"#,
            20,
            str![[r#"
mixed = [
    1,
    "two",
    true,
    3.14
]

"#]],
        );
    }

    #[test]
    fn multiple_arrays_same_section() {
        // Each array should be evaluated independently
        valid(
            r#"[pkg]
a = [1, 2, 3]
b = [4, 5, 6, 7, 8]
"#,
            15,
            str![[r#"
[pkg]
a = [1, 2, 3]
b = [
    4,
    5,
    6,
    7,
    8
]

"#]],
        );
    }

    #[test]
    fn array_at_start_of_file() {
        // No newline before - line starts at index 0
        valid(
            r#"x = ["a", "b", "c"]
"#,
            15,
            str![[r#"
x = [
    "a",
    "b",
    "c"
]

"#]],
        );
    }

    #[test]
    fn empty_string_elements() {
        valid(
            r#"x = ["", "a", ""]
"#,
            12,
            str![[r#"
x = [
    "",
    "a",
    ""
]

"#]],
        );
    }

    #[test]
    fn integer_array() {
        valid(
            r#"nums = [1, 2, 3, 4, 5]
"#,
            15,
            str![[r#"
nums = [
    1,
    2,
    3,
    4,
    5
]

"#]],
        );
    }

    #[test]
    fn boolean_array() {
        valid(
            r#"flags = [true, false, true]
"#,
            20,
            str![[r#"
flags = [
    true,
    false,
    true
]

"#]],
        );
    }

    #[test]
    fn float_array() {
        valid(
            r#"values = [1.5, 2.25, 3.125]
"#,
            20,
            str![[r#"
values = [
    1.5,
    2.25,
    3.125
]

"#]],
        );
    }

    #[test]
    fn nested_only_inner_exceeds() {
        // At width 12, both outer and inner arrays exceed max_width
        valid(
            r#"x = [[1, 2, 3, 4]]
"#,
            12,
            str![[r#"
x = [
    [
        1,
        2,
        3,
        4
    ]
]

"#]],
        );
    }

    #[test]
    fn very_long_key_array_still_reflows() {
        // Even if key alone is long, array should still reflow
        valid(
            r#"this_is_a_very_long_key = [1]
"#,
            20,
            str![[r#"
this_is_a_very_long_key = [
    1
]

"#]],
        );
    }
}
