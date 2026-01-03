use std::borrow::Cow;

use unicode_width::UnicodeWidthChar;

use crate::toml::TokenIndices;
use crate::toml::TokenKind;
use crate::toml::TomlToken;
use crate::toml::TomlTokens;

/// Display width of array brackets: `[` and `]`.
const ARRAY_BRACKETS_WIDTH: usize = 2;

/// Display width of comma plus space: `, `.
const COMMA_SPACE_WIDTH: usize = 2;

/// Normalize array layouts based on `array_width`.
///
/// - Expands horizontal arrays to vertical when they exceed `array_width`
/// - Collapses vertical arrays to horizontal when they fit within `array_width`
/// - Normalizes mixed-style arrays to the appropriate format
/// - Preserves arrays containing comments (no collapse)
///
/// Uses incremental depth tracking for O(n) complexity instead of
/// rescanning from the start for each array.
#[tracing::instrument]
pub fn reflow_arrays(tokens: &mut TomlTokens<'_>, array_width: usize, tab_spaces: usize) {
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
                if let Some(action) =
                    determine_array_action(tokens, i, inline_table_depth, array_width, tab_spaces)
                {
                    apply_array_action(tokens, i, action, tab_spaces, nesting_depth);
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

/// Actions that can be performed on an array.
enum ArrayAction {
    /// Collapse vertical array to horizontal
    Collapse { close: usize },
    /// Expand horizontal array to vertical
    Expand { close: usize },
    /// Normalize mixed-style to proper vertical (collapse then expand)
    Normalize { close: usize },
}

/// Determine what action to take on an array at the given index.
fn determine_array_action(
    tokens: &TomlTokens<'_>,
    open: usize,
    inline_table_depth: usize,
    array_width: usize,
    tab_spaces: usize,
) -> Option<ArrayAction> {
    // Skip arrays inside inline tables
    if inline_table_depth > 0 {
        return None;
    }

    let close = find_array_close(tokens, open)?;

    // Skip malformed arrays (no actual closing bracket)
    if tokens.tokens[close].raw != "]" {
        return None;
    }

    if is_array_vertical(tokens, open, close) {
        determine_vertical_array_action(tokens, open, close, array_width, tab_spaces)
    } else {
        determine_horizontal_array_action(tokens, open, close, array_width, tab_spaces)
    }
}

/// Determine action for a vertical or mixed-style array.
fn determine_vertical_array_action(
    tokens: &TomlTokens<'_>,
    open: usize,
    close: usize,
    array_width: usize,
    tab_spaces: usize,
) -> Option<ArrayAction> {
    if should_collapse_array(tokens, open, close, array_width, tab_spaces) {
        return Some(ArrayAction::Collapse { close });
    }

    // Mixed-style without comments should be normalized
    if !has_comments(tokens, open, close) && !is_properly_vertical(tokens, open, close) {
        return Some(ArrayAction::Normalize { close });
    }

    None
}

/// Determine action for a horizontal array.
fn determine_horizontal_array_action(
    tokens: &TomlTokens<'_>,
    open: usize,
    close: usize,
    array_width: usize,
    tab_spaces: usize,
) -> Option<ArrayAction> {
    if should_reflow_array(tokens, open, close, array_width, tab_spaces) {
        Some(ArrayAction::Expand { close })
    } else {
        None
    }
}

/// Apply the determined action to an array.
fn apply_array_action(
    tokens: &mut TomlTokens<'_>,
    open: usize,
    action: ArrayAction,
    tab_spaces: usize,
    nesting_depth: usize,
) {
    match action {
        ArrayAction::Collapse { close } => {
            collapse_array_to_horizontal(tokens, open, close);
        }
        ArrayAction::Expand { close } => {
            reflow_array_to_vertical(tokens, open, close, tab_spaces, nesting_depth);
        }
        ArrayAction::Normalize { close } => {
            collapse_array_to_horizontal(tokens, open, close);
            let new_close = find_array_close(tokens, open).unwrap_or(open);
            reflow_array_to_vertical(tokens, open, new_close, tab_spaces, nesting_depth);
        }
    }
}

/// Check if a horizontal array should be expanded to vertical layout.
fn should_reflow_array(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    array_width: usize,
    tab_spaces: usize,
) -> bool {
    // Calculate line width including the array
    let line_start = find_line_start(tokens, open_index);
    let line_width: usize = tokens.tokens[line_start..=close_index]
        .iter()
        .map(|t| token_width(&t.raw, tab_spaces))
        .sum();

    line_width > array_width
}

/// Check if array already has vertical layout (contains newlines).
fn is_array_vertical(tokens: &TomlTokens<'_>, open_index: usize, close_index: usize) -> bool {
    tokens.tokens[open_index..=close_index]
        .iter()
        .any(|t| t.kind == TokenKind::Newline)
}

/// Check if a vertical array is properly formatted (one element per line).
///
/// Returns true if:
/// - Opens with `[\n`
/// - Each element is on its own line
/// - Closes with `]` on its own line
fn is_properly_vertical(tokens: &TomlTokens<'_>, open_index: usize, close_index: usize) -> bool {
    // Must have newline immediately after open bracket
    if open_index + 1 >= close_index {
        return true; // Empty array is fine
    }
    if tokens.tokens[open_index + 1].kind != TokenKind::Newline {
        return false;
    }

    // Check that each value separator (comma) is followed by newline (possibly with whitespace first)
    let mut local_depth = 0;
    for i in (open_index + 1)..close_index {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => local_depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => local_depth -= 1,
            TokenKind::ValueSep if local_depth == 0 => {
                // After comma, we should have optional whitespace then newline
                let mut j = i + 1;
                while j < close_index && tokens.tokens[j].kind == TokenKind::Whitespace {
                    j += 1;
                }
                if j < close_index && tokens.tokens[j].kind != TokenKind::Newline {
                    return false;
                }
            }
            _ => {}
        }
    }

    true
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
///
/// Uses Unicode width for accurate display column counting:
/// - CJK characters are double-width (2 columns)
/// - Emoji are typically double-width
/// - Zero-width joiners and combining characters are 0 width
/// - Tabs expand to `tab_spaces` columns
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

    clear_post_comma_whitespace(tokens, open_index, close_index);
    let close_index = ensure_trailing_comma(tokens, open_index, close_index);

    let insertions =
        collect_vertical_insertions(tokens, open_index, close_index, &indent, &close_indent);

    apply_newline_insertions(tokens, insertions);
    tokens.trim_empty_whitespace();
}

/// Ensure array has a trailing comma after the last element.
///
/// Returns the updated close index (incremented by 1 if comma was inserted).
fn ensure_trailing_comma(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> usize {
    // Find the last non-whitespace token before close bracket
    let mut last_value_index = None;
    let mut local_depth = 0;

    for i in (open_index + 1)..close_index {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => local_depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => {
                local_depth -= 1;
                // When closing brings us back to depth 0, this bracket is the last value
                if local_depth == 0 {
                    last_value_index = Some(i);
                }
            }
            TokenKind::Whitespace | TokenKind::Newline => {}
            TokenKind::ValueSep if local_depth == 0 => {
                // If this is a trailing comma, we're done
                if is_trailing_comma(tokens, i, close_index) {
                    return close_index;
                }
            }
            _ if local_depth == 0 => {
                last_value_index = Some(i);
            }
            _ => {}
        }
    }

    // If we found a last value and there's no trailing comma, add one
    if let Some(idx) = last_value_index {
        tokens.tokens.insert(idx + 1, TomlToken::VAL_SEP);
        return close_index + 1;
    }

    close_index
}

/// Clear whitespace immediately after commas to prepare for reformatting.
fn clear_post_comma_whitespace(tokens: &mut TomlTokens<'_>, open_index: usize, close_index: usize) {
    let mut local_depth = 0;
    for i in (open_index + 1)..close_index {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => local_depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => local_depth -= 1,
            TokenKind::ValueSep if local_depth == 0 => {
                if i + 1 < close_index && tokens.tokens[i + 1].kind == TokenKind::Whitespace {
                    tokens.tokens[i + 1] = TomlToken::EMPTY;
                }
            }
            _ => {}
        }
    }
}

/// Collect positions where newline + indent should be inserted.
fn collect_vertical_insertions<'a>(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    indent: &'a str,
    close_indent: &'a str,
) -> Vec<(usize, &'a str)> {
    let mut insertions = vec![(open_index + 1, indent)];

    let mut local_depth = 0;
    for i in (open_index + 1)..close_index {
        match tokens.tokens[i].kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => local_depth += 1,
            TokenKind::ArrayClose | TokenKind::InlineTableClose => local_depth -= 1,
            TokenKind::ValueSep if local_depth == 0 => {
                if !is_trailing_comma(tokens, i, close_index) {
                    insertions.push((i + 1, indent));
                }
            }
            _ => {}
        }
    }

    insertions.push((close_index, close_indent));
    insertions
}

/// Apply newline + indent insertions in reverse order to maintain indices.
fn apply_newline_insertions(tokens: &mut TomlTokens<'_>, insertions: Vec<(usize, &str)>) {
    for (index, indent) in insertions.into_iter().rev() {
        if !indent.is_empty() {
            tokens.tokens.insert(
                index,
                TomlToken {
                    kind: TokenKind::Whitespace,
                    encoding: None,
                    decoded: None,
                    scalar: None,
                    raw: Cow::Owned(indent.to_owned()),
                },
            );
        }
        tokens.tokens.insert(index, TomlToken::NL);
    }
}

/// Check if a comma is a trailing comma (only whitespace/newlines between it and close bracket).
fn is_trailing_comma(tokens: &TomlTokens<'_>, comma_index: usize, close_index: usize) -> bool {
    tokens.tokens[(comma_index + 1)..close_index]
        .iter()
        .all(|t| matches!(t.kind, TokenKind::Whitespace | TokenKind::Newline))
}

/// Check if array contains any comments.
fn has_comments(tokens: &TomlTokens<'_>, open_index: usize, close_index: usize) -> bool {
    tokens.tokens[open_index..=close_index]
        .iter()
        .any(|t| t.kind == TokenKind::Comment)
}

/// Calculate the width of an array if collapsed to horizontal layout.
///
/// Returns the total line width including the key prefix, excluding trailing comma.
fn calculate_collapsed_width(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    tab_spaces: usize,
) -> usize {
    let prefix_width = calculate_prefix_width(tokens, open_index, tab_spaces);
    let content_width = calculate_content_width(tokens, open_index, close_index, tab_spaces);
    prefix_width + content_width
}

/// Calculate width of everything before the array (key, equals, spaces).
fn calculate_prefix_width(tokens: &TomlTokens<'_>, open_index: usize, tab_spaces: usize) -> usize {
    let line_start = find_line_start(tokens, open_index);
    tokens.tokens[line_start..open_index]
        .iter()
        .map(|t| token_width(&t.raw, tab_spaces))
        .sum()
}

/// Calculate width of array content when collapsed (excludes newlines, indents, trailing comma).
fn calculate_content_width(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    tab_spaces: usize,
) -> usize {
    let content_width = ((open_index + 1)..close_index).fold((0, false), |(width, after_nl), i| {
        match collapsed_token_contribution(tokens, i, close_index, tab_spaces, after_nl) {
            Some((w, new_after_nl)) => (width + w, new_after_nl),
            None => (width, false),
        }
    });
    ARRAY_BRACKETS_WIDTH + content_width.0
}

/// Calculate a token's width contribution when collapsed.
///
/// Returns `Some((width, after_newline))` for tokens that contribute,
/// `None` for tokens that should be skipped (indent whitespace after newline).
fn collapsed_token_contribution(
    tokens: &TomlTokens<'_>,
    index: usize,
    close_index: usize,
    tab_spaces: usize,
    after_newline: bool,
) -> Option<(usize, bool)> {
    let token = &tokens.tokens[index];

    match token.kind {
        TokenKind::Newline => Some((0, true)),
        TokenKind::Whitespace if after_newline => None, // Skip indent
        TokenKind::ValueSep if is_trailing_comma(tokens, index, close_index) => Some((0, false)),
        TokenKind::ValueSep => Some((COMMA_SPACE_WIDTH, false)),
        _ => Some((token_width(&token.raw, tab_spaces), false)),
    }
}

/// Check if a vertical/mixed array should be collapsed to horizontal.
fn should_collapse_array(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    array_width: usize,
    tab_spaces: usize,
) -> bool {
    // Don't collapse arrays with comments
    if has_comments(tokens, open_index, close_index) {
        return false;
    }

    // Calculate collapsed width
    let collapsed_width = calculate_collapsed_width(tokens, open_index, close_index, tab_spaces);

    collapsed_width <= array_width
}

/// Collapse a vertical/mixed array to horizontal layout.
fn collapse_array_to_horizontal(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) {
    // Each function returns the updated close index after mutations
    let close = remove_newlines_and_indents(tokens, open_index, close_index);
    let close = remove_pre_comma_whitespace_and_trailing(tokens, open_index, close);
    normalize_comma_spacing(tokens, open_index, close);
}

/// Remove newlines and their following indent whitespace from array.
///
/// Returns the updated close index after removals.
fn remove_newlines_and_indents(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> usize {
    let mut removals: Vec<usize> = Vec::new();
    let mut i = open_index + 1;

    while i < close_index {
        if tokens.tokens[i].kind == TokenKind::Newline {
            removals.push(i);
            if i + 1 < close_index && tokens.tokens[i + 1].kind == TokenKind::Whitespace {
                removals.push(i + 1);
            }
        }
        i += 1;
    }

    let removal_count = removals.len();
    for idx in removals.into_iter().rev() {
        tokens.tokens.remove(idx);
    }

    close_index - removal_count
}

/// Remove whitespace before commas and trailing comma.
///
/// Returns the updated close index after removals.
fn remove_pre_comma_whitespace_and_trailing(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    mut close: usize,
) -> usize {
    let mut i = open_index + 1;

    while i < close {
        if is_whitespace_before_comma(tokens, i, close) {
            tokens.tokens.remove(i);
            close -= 1;
            continue;
        }

        if tokens.tokens[i].kind == TokenKind::ValueSep && is_trailing_comma(tokens, i, close) {
            tokens.tokens.remove(i);
            close -= 1;
            continue;
        }

        i += 1;
    }

    close
}

/// Check if token at index is whitespace immediately before a comma.
fn is_whitespace_before_comma(tokens: &TomlTokens<'_>, index: usize, close_index: usize) -> bool {
    tokens.tokens[index].kind == TokenKind::Whitespace
        && index + 1 < close_index
        && tokens.tokens[index + 1].kind == TokenKind::ValueSep
}

/// Normalize spacing after commas to exactly one space.
fn normalize_comma_spacing(tokens: &mut TomlTokens<'_>, open_index: usize, mut close: usize) {
    let mut i = open_index + 1;

    while i < close {
        if tokens.tokens[i].kind == TokenKind::ValueSep
            && i + 1 < close
            && ensure_single_space_after(tokens, i)
        {
            close += 1; // Token was inserted
            i += 1; // Skip past inserted space
        }
        i += 1;
    }
}

/// Ensure exactly one space exists after the token at index.
///
/// Returns `true` if a new token was inserted, `false` if existing token was replaced or no change.
fn ensure_single_space_after(tokens: &mut TomlTokens<'_>, index: usize) -> bool {
    let next_index = index + 1;
    if next_index >= tokens.len() {
        return false;
    }

    let next = &tokens.tokens[next_index];
    if next.kind == TokenKind::Whitespace {
        if next.raw != " " {
            tokens.tokens[next_index] = make_single_space_token();
        }
        false
    } else {
        tokens.tokens.insert(next_index, make_single_space_token());
        true
    }
}

/// Create a single space whitespace token.
fn make_single_space_token() -> TomlToken<'static> {
    TomlToken {
        kind: TokenKind::Whitespace,
        encoding: None,
        decoded: None,
        scalar: None,
        raw: Cow::Borrowed(" "),
    }
}

/// Create indentation string for the given nesting depth.
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
    "baz",
]

"#]],
        );
    }

    #[test]
    fn already_vertical_not_modified() {
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
        valid(
            r#"matrix = [[1, 2, 3], [4, 5, 6]]
"#,
            20,
            str![[r#"
matrix = [
    [1, 2, 3],
    [4, 5, 6],
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_array() {
        valid(
            r#"x = [[[1]]]
"#,
            5,
            str![[r#"
x = [
    [
        [
            1,
        ],
    ],
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_partial_reflow() {
        valid(
            r#"x = [[[1]]]
"#,
            10,
            str![[r#"
x = [
    [[1]],
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
    {name = "bar"},
]

"#]],
        );
    }

    #[test]
    fn empty_array_not_reflowed() {
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
    2,
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
    1,
]

"#]],
        );
    }

    #[test]
    fn max_width_max_reflows_nothing() {
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
        valid(
            r#"deps = [{name = "very-long-name", version = "1.0.0", features = ["a", "b"]}]
"#,
            40,
            str![[r#"
deps = [
    {name = "very-long-name", version = "1.0.0", features = ["a", "b"]},
]

"#]],
        );
    }

    #[test]
    fn inline_table_containing_array() {
        valid(
            r#"dep = [{features = ["a", "b", "c"]}]
"#,
            20,
            str![[r#"
dep = [
    {features = ["a", "b", "c"]},
]

"#]],
        );
    }

    #[test]
    fn nested_inline_tables() {
        valid(
            r#"items = [{outer = {inner = "value"}}]
"#,
            20,
            str![[r#"
items = [
    {outer = {inner = "value"}},
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
    "bar",
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
        valid(
            r#"deps = ["this-is-a-very-long-package-name"]
"#,
            20,
            str![[r#"
deps = [
    "this-is-a-very-long-package-name",
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
    "formatter",
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
    "한국어",
]

"#]],
        );
    }

    #[test]
    fn multiline_string_in_array() {
        // Newlines inside string literals don't count as array being vertical
        // Input is horizontal (no Newline tokens in array structure)
        // but exceeds max_width so should reflow to vertical
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
""",
]

"#]],
        );
    }

    #[test]
    fn vertical_multiline_string_collapses_when_fits() {
        // The multiline string content (embedded newlines) must be preserved
        valid(
            r#"x = [
    """
multi
""",
]
"#,
            80,
            // Collapsed form: array is horizontal but string still spans lines
            str![[r#"
x = ["""
multi
"""]

"#]],
        );
    }

    #[test]
    fn multiline_literal_string_preserved() {
        // Triple single quotes (''') should also be handled correctly
        valid(
            r#"x = [
    '''
literal
''',
]
"#,
            80,
            str![[r#"
x = ['''
literal
''']

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
        // "foo.bar.baz = [\"a\", \"b\"]" = 24 chars
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            23,
            str![[r#"
foo.bar.baz = [
    "a",
    "b",
]

"#]],
        );
    }

    #[test]
    fn dotted_key_at_exact_width() {
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
    "y",
]

"#]],
        );
    }

    #[test]
    fn literal_strings() {
        valid(
            r#"paths = ['foo', 'bar']
"#,
            15,
            str![[r#"
paths = [
    'foo',
    'bar',
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
    3.14,
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
    8,
]

"#]],
        );
    }

    #[test]
    fn array_at_start_of_file() {
        valid(
            r#"x = ["a", "b", "c"]
"#,
            15,
            str![[r#"
x = [
    "a",
    "b",
    "c",
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
    "",
]

"#]],
        );
    }

    #[test]
    fn nested_only_inner_exceeds() {
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
        4,
    ],
]

"#]],
        );
    }

    #[test]
    fn very_long_key_array_still_reflows() {
        valid(
            r#"this_is_a_very_long_key = [1]
"#,
            20,
            str![[r#"
this_is_a_very_long_key = [
    1,
]

"#]],
        );
    }

    // Collapse tests

    #[test]
    fn vertical_collapses_when_fits() {
        valid(
            r#"x = [
    "a",
    "b",
]
"#,
            40,
            str![[r#"
x = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn vertical_stays_when_too_wide() {
        // Vertical array that doesn't fit should stay vertical
        valid(
            r#"x = [
    "aaa",
    "bbb",
]
"#,
            10,
            str![[r#"
x = [
    "aaa",
    "bbb",
]

"#]],
        );
    }

    #[test]
    fn mixed_style_collapses_when_fits() {
        valid(
            r#"x = ["a", "b",
    "c"]
"#,
            40,
            str![[r#"
x = ["a", "b", "c"]

"#]],
        );
    }

    #[test]
    fn mixed_style_normalizes_when_too_wide() {
        // Mixed-style array that doesn't fit should normalize to vertical
        valid(
            r#"x = ["aaa", "bbb",
    "ccc"]
"#,
            10,
            str![[r#"
x = [
    "aaa",
    "bbb",
    "ccc",
]

"#]],
        );
    }

    #[test]
    fn vertical_with_comment_stays_vertical() {
        // Don't collapse arrays with comments
        valid(
            r#"x = [
    "a", # comment
    "b",
]
"#,
            80,
            str![[r#"
x = [
    "a", # comment
    "b",
]

"#]],
        );
    }

    #[test]
    fn mixed_style_with_comment_preserved() {
        // Mixed-style arrays with comments are preserved as-is
        // (not normalized) to avoid semantic changes from comment displacement
        valid(
            r#"x = ["a", "b", # comment
    "c",
]
"#,
            80,
            str![[r#"
x = ["a", "b", # comment
    "c",
]

"#]],
        );
    }

    #[test]
    fn grouped_comments_preserved_on_overflow() {
        // When any grouped line exceeds max_width, rustfmt splits ALL groups.
        // We preserve grouping to avoid losing semantic structure.
        valid(
            r#"deps = [
    "a", "b", "c",
    "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", # comment about this group
    "x", "y", "z", # fits
]
"#,
            60,
            str![[r#"
deps = [
    "a", "b", "c",
    "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", # comment about this group
    "x", "y", "z", # fits
]

"#]],
        );
    }

    #[test]
    fn standalone_comments_preserved_on_collapse() {
        // Standalone comments between elements would be moved to trailing
        // position on previous element during collapse, changing semantics.
        valid(
            r#"deps = [
    "a",
    "b",
    # comment about elements below
    "c",
    "d",
]
"#,
            200,
            str![[r#"
deps = [
    "a",
    "b",
    # comment about elements below
    "c",
    "d",
]

"#]],
        );
    }

    #[test]
    fn nested_vertical_collapses() {
        valid(
            r#"x = [
    [
        1
    ],
    [
        2
    ],
]
"#,
            40,
            str![[r#"
x = [[1], [2]]

"#]],
        );
    }

    #[test]
    fn collapse_removes_trailing_comma() {
        // Trailing comma should be removed when collapsing
        valid(
            r#"x = [
    "a",
    "b",
]
"#,
            40,
            str![[r#"
x = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn collapse_normalizes_spacing() {
        // Collapsed array should have consistent spacing
        valid(
            r#"x = [
    "a"  ,
    "b"  ,
]
"#,
            40,
            str![[r#"
x = ["a", "b"]

"#]],
        );
    }

    // Unicode width edge case tests

    #[test]
    fn cjk_double_width_causes_reflow() {
        // `a = ["日"]` = 9 codepoints but 10 display columns
        // At max_width=9: should reflow because display width (10) > 9
        valid(
            r#"a = ["日"]
"#,
            9,
            str![[r#"
a = [
    "日",
]

"#]],
        );
    }

    #[test]
    fn cjk_double_width_fits_at_correct_width() {
        // `a = ["日"]` = 10 display columns
        // At max_width=10: should NOT reflow
        valid(
            r#"a = ["日"]
"#,
            10,
            str![[r#"
a = ["日"]

"#]],
        );
    }

    #[test]
    fn emoji_double_width_causes_reflow() {
        // `a = ["🎉"]` = 9 codepoints but 10 display columns
        valid(
            r#"a = ["🎉"]
"#,
            9,
            str![[r#"
a = [
    "🎉",
]

"#]],
        );
    }

    #[test]
    fn emoji_double_width_fits_at_correct_width() {
        // `a = ["🎉"]` = 10 display columns
        valid(
            r#"a = ["🎉"]
"#,
            10,
            str![[r#"
a = ["🎉"]

"#]],
        );
    }

    #[test]
    fn combining_character_zero_width() {
        // "é" as e + combining acute (U+0301) is 2 codepoints but 1 display column
        // `a = ["é"]` with combining = 10 codepoints but 9 display columns
        // At max_width=9: should NOT reflow (display width fits)
        valid(
            "a = [\"e\u{0301}\"]\n",
            9,
            // Expected output preserves decomposed form (e + combining acute)
            "a = [\"e\u{0301}\"]\n",
        );
    }

    #[test]
    fn combining_character_reflows_at_boundary() {
        // At max_width=8: should reflow (display width 9 > 8)
        valid(
            "a = [\"e\u{0301}\"]\n",
            8,
            // Expected output preserves decomposed form (e + combining acute)
            "a = [\n    \"e\u{0301}\",\n]\n",
        );
    }

    #[test]
    fn vertical_cjk_collapses_at_correct_width() {
        // Collapsed: `x = ["日", "月"]` = 16 display columns
        valid(
            r#"x = [
    "日",
    "月",
]
"#,
            16,
            str![[r#"
x = ["日", "月"]

"#]],
        );
    }

    #[test]
    fn vertical_cjk_stays_vertical_when_too_wide() {
        // Collapsed: `x = ["日", "月"]` = 16 display columns
        // At max_width=15: should stay vertical
        valid(
            r#"x = [
    "日",
    "月",
]
"#,
            15,
            str![[r#"
x = [
    "日",
    "月",
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_within_limit() {
        let nested = "x = [[[[[[[[[[1]]]]]]]]]]\n";
        valid(
            nested,
            5,
            str![[r#"
x = [
    [
        [
            [
                [
                    [
                        [
                            [
                                [
                                    [
                                        1,
                                    ],
                                ],
                            ],
                        ],
                    ],
                ],
            ],
        ],
    ],
]

"#]],
        );
    }

    // Tab handling tests

    #[test]
    fn tabs_in_array_counted_as_tab_spaces() {
        // Tab expands to 4 columns (DEFAULT_TAB_SPACES)
        // "x = [\t1]" = 1+1+1+1+1+4+1+1 = 11 display columns
        // At max_width=11: should NOT reflow
        valid("x = [\t1]\n", 11, "x = [\t1]\n");
    }

    #[test]
    fn tabs_in_array_cause_reflow_at_boundary() {
        // "x = [\t1]" = 11 display columns
        // At max_width=10: should reflow
        // Note: tab inside content is preserved
        valid("x = [\t1]\n", 10, "x = [\n    \t1,\n]\n");
    }

    #[test]
    fn tabs_between_elements_normalized_on_collapse() {
        // "x = [1, 2]" = 10 columns < 40
        valid(
            "x = [\n\t1,\n\t2,\n]\n",
            40,
            str![[r#"
x = [1, 2]

"#]],
        );
    }

    #[test]
    fn multiple_tabs_expand_correctly() {
        valid(
            "x = [\t\t1]\n",
            12,
            str![[r#"
x = [
    		1,
]

"#]],
        );
    }

    // Deeply nested mixed collapse/expand tests

    #[test]
    fn vertical_outer_with_long_horizontal_inner_expands_inner() {
        // Outer is already vertical, inner is horizontal but exceeds width
        // "    [1, 2, 3, 4, 5]," = 20 columns > 15, inner expands
        valid(
            r#"x = [
    [1, 2, 3, 4, 5],
]
"#,
            15,
            str![[r#"
x = [
    [
        1,
        2,
        3,
        4,
        5,
    ],
]

"#]],
        );
    }

    #[test]
    fn vertical_outer_with_short_horizontal_inner_collapses() {
        valid(
            r#"x = [
    [1, 2],
]
"#,
            40,
            str![[r#"
x = [[1, 2]]

"#]],
        );
    }

    #[test]
    fn horizontal_outer_fits_stays_horizontal() {
        valid(
            r#"x = [[1], [2]]
"#,
            20,
            str![[r#"
x = [[1], [2]]

"#]],
        );
    }

    #[test]
    fn outer_expands_inner_fits() {
        valid(
            r#"x = [[1], [2]]
"#,
            10,
            str![[r#"
x = [
    [1],
    [2],
]

"#]],
        );
    }

    #[test]
    fn outer_expands_inner_also_expands() {
        valid(
            r#"x = [[1, 2, 3], [4, 5, 6]]
"#,
            10,
            str![[r#"
x = [
    [
        1,
        2,
        3,
    ],
    [
        4,
        5,
        6,
    ],
]

"#]],
        );
    }

    #[test]
    fn mixed_nesting_all_inner_fit() {
        valid(
            r#"x = [[1], [2], [3]]
"#,
            15,
            str![[r#"
x = [
    [1],
    [2],
    [3],
]

"#]],
        );
    }

    #[test]
    fn mixed_nesting_one_inner_expands() {
        valid(
            r#"x = [[1], [2, 3, 4, 5], [6]]
"#,
            15,
            str![[r#"
x = [
    [1],
    [
        2,
        3,
        4,
        5,
    ],
    [6],
]

"#]],
        );
    }

    #[test]
    fn three_level_nesting_all_expand() {
        // "    [[1, 2]]" = 10 columns > 5, middle expands
        // "        [1, 2]" = 11 columns > 5, inner expands
        valid(
            r#"x = [[[1, 2]]]
"#,
            5,
            str![[r#"
x = [
    [
        [
            1,
            2,
        ],
    ],
]

"#]],
        );
    }

    #[test]
    fn three_level_nesting_small_width() {
        // "    [[1]]" = 10 columns > 8, middle expands
        valid(
            r#"x = [[[1]]]
"#,
            8,
            str![[r#"
x = [
    [
        [
            1,
        ],
    ],
]

"#]],
        );
    }

    #[test]
    fn empty_vertical_array_collapses() {
        valid(
            r#"x = [
]
"#,
            80,
            str![[r#"
x = []

"#]],
        );
    }

    #[test]
    fn empty_vertical_array_with_whitespace_collapses() {
        valid(
            r#"x = [

]
"#,
            80,
            str![[r#"
x = []

"#]],
        );
    }

    #[test]
    fn long_string_width_at_boundary() {
        valid(
            r#"x = ["abcdefghij"]
"#,
            18,
            str![[r#"
x = ["abcdefghij"]

"#]],
        );
    }

    #[test]
    fn long_string_width_causes_reflow() {
        valid(
            r#"x = ["abcdefghij"]
"#,
            17,
            str![[r#"
x = [
    "abcdefghij",
]

"#]],
        );
    }

    #[test]
    fn string_with_special_chars() {
        // String with various special chars that don't need escaping
        // `x = ["a-b_c.d"]` = 15 columns
        // At max_width=14: should reflow
        valid(
            r#"x = ["a-b_c.d"]
"#,
            14,
            str![[r#"
x = [
    "a-b_c.d",
]

"#]],
        );
    }

    #[test]
    fn array_with_only_whitespace_preserved() {
        valid(
            r#"x = [   ]
"#,
            20,
            str![[r#"
x = [   ]

"#]],
        );
    }
}
