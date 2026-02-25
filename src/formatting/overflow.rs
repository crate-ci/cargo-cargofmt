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
/// - Normalizes mixed-style arrays to proper vertical format
/// - Preserves arrays containing comments (no collapse, but normalizes layout)
/// - Comments are preserved in their relative positions during normalization
///
/// Uses incremental depth tracking for O(n) complexity instead of
/// rescanning from the start for each array.
#[tracing::instrument]
pub fn reflow_arrays(
    tokens: &mut TomlTokens<'_>,
    array_width: usize,
    element_threshold: usize,
    tab_spaces: usize,
) {
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
                process_array(
                    tokens,
                    i,
                    inline_table_depth,
                    nesting_depth,
                    array_width,
                    element_threshold,
                    tab_spaces,
                );
                nesting_depth += 1;
            }
            TokenKind::ArrayClose => {
                nesting_depth = nesting_depth.saturating_sub(1);
            }
            _ => {}
        }
    }
}

/// Process a single array: determine and apply reflow action.
fn process_array(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    inline_table_depth: usize,
    nesting_depth: usize,
    array_width: usize,
    element_threshold: usize,
    tab_spaces: usize,
) {
    if let Some(action) = determine_array_action(
        tokens,
        open_index,
        inline_table_depth,
        array_width,
        element_threshold,
        tab_spaces,
    ) {
        apply_array_action(
            tokens,
            open_index,
            action,
            tab_spaces,
            nesting_depth,
            array_width,
        );
    }
}

/// Actions that can be performed on an array.
enum ArrayAction {
    /// Collapse vertical array to horizontal
    Collapse { close: usize },
    /// Collapse elements to horizontal, but keep closing bracket on new line (for trailing comment)
    CollapseWithComment { close: usize },
    /// Expand horizontal array to vertical
    Expand { close: usize },
    /// Normalize mixed-style to proper vertical (collapse then expand)
    Normalize { close: usize },
    /// Reflow with horizontal grouping (comments act as line-enders)
    ReflowGrouped { close: usize },
}

/// Determine what action to take on an array at the given index.
fn determine_array_action(
    tokens: &TomlTokens<'_>,
    open: usize,
    inline_table_depth: usize,
    array_width: usize,
    element_threshold: usize,
    tab_spaces: usize,
) -> Option<ArrayAction> {
    // Skip arrays inside inline tables
    if inline_table_depth > 0 {
        return None;
    }

    let close = find_array_close(tokens, open)?;

    if is_array_vertical(tokens, open, close) {
        determine_vertical_array_action(tokens, open, close, array_width, tab_spaces)
    } else {
        determine_horizontal_array_action(
            tokens,
            open,
            close,
            array_width,
            element_threshold,
            tab_spaces,
        )
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
    let comment_pos = comment_position(tokens, open, close);

    if should_collapse_array(tokens, open, close, array_width, tab_spaces) {
        return match comment_pos {
            CommentPosition::LastElementOnly => Some(ArrayAction::CollapseWithComment { close }),
            _ => Some(ArrayAction::Collapse { close }),
        };
    }

    // Check if array is already properly formatted
    if is_properly_vertical(tokens, open, close) {
        return None;
    }

    // Mixed-style arrays need normalization
    // Rustfmt behavior:
    // - Uniform element widths: horizontal grouping allowed
    // - Mixed element widths: one element per line
    let uniform_widths = has_uniform_element_widths(tokens, open, close);

    match (comment_pos, uniform_widths) {
        // Comments on non-last element with uniform widths: horizontal grouping
        (CommentPosition::NonLastElement | CommentPosition::BeforeClose, true) => {
            Some(ArrayAction::ReflowGrouped { close })
        }
        // Mixed widths or no special comments: one element per line
        _ => Some(ArrayAction::Normalize { close }),
    }
}

/// Determine action for a horizontal array.
fn determine_horizontal_array_action(
    tokens: &TomlTokens<'_>,
    open: usize,
    close: usize,
    array_width: usize,
    element_threshold: usize,
    tab_spaces: usize,
) -> Option<ArrayAction> {
    if should_reflow_array(
        tokens,
        open,
        close,
        array_width,
        element_threshold,
        tab_spaces,
    ) {
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
    array_width: usize,
) {
    match action {
        ArrayAction::Collapse { close } => {
            collapse_array_to_horizontal(tokens, open, close);
        }
        ArrayAction::CollapseWithComment { close } => {
            collapse_with_trailing_comment(tokens, open, close, nesting_depth, tab_spaces);
        }
        ArrayAction::Expand { close } => {
            reflow_array_to_vertical(tokens, open, close, tab_spaces, nesting_depth);
        }
        ArrayAction::Normalize { close } => {
            collapse_array_to_horizontal(tokens, open, close);
            let new_close = find_array_close(tokens, open).unwrap_or(open);
            reflow_array_to_vertical(tokens, open, new_close, tab_spaces, nesting_depth);
        }
        ArrayAction::ReflowGrouped { close } => {
            reflow_grouped(tokens, open, close, tab_spaces, nesting_depth, array_width);
        }
    }
}

/// Check if a horizontal array should be expanded to vertical layout.
fn should_reflow_array(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    array_width: usize,
    element_threshold: usize,
    tab_spaces: usize,
) -> bool {
    // Calculate line width including the array
    let line_start = find_line_start(tokens, open_index);
    let line_width: usize = tokens.tokens[line_start..=close_index]
        .iter()
        .map(|t| token_width(&t.raw, tab_spaces))
        .sum();

    if line_width > array_width {
        return true;
    }

    let widths = collect_element_widths(tokens, open_index, close_index);
    widths.iter().any(|&w| w > element_threshold)
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
    // Also check for standalone comments (which require regrouping)
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
            TokenKind::Comment if local_depth == 0 => {
                // Check if this is a standalone comment (preceded by newline+whitespace)
                if is_standalone_comment(tokens, i, open_index) {
                    return false; // Needs regrouping
                }
            }
            _ => {}
        }
    }

    true
}

/// Check if a comment at the given index is a standalone comment (on its own line).
fn is_standalone_comment(tokens: &TomlTokens<'_>, comment_index: usize, open_index: usize) -> bool {
    if comment_index <= open_index + 1 {
        return false;
    }

    // Look backwards to find if preceded by newline + optional whitespace
    let mut i = comment_index - 1;
    if tokens.tokens[i].kind == TokenKind::Whitespace {
        if i > open_index + 1 {
            i -= 1;
        } else {
            return false;
        }
    }

    tokens.tokens[i].kind == TokenKind::Newline
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

/// Check if array elements have uniform widths.
///
/// Rustfmt uses horizontal grouping only when elements have uniform widths.
/// When widths are mixed, it formats one element per line.
fn has_uniform_element_widths(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> bool {
    let widths = collect_element_widths(tokens, open_index, close_index);
    all_widths_equal(&widths)
}

/// Check if all widths in a slice are equal.
fn all_widths_equal(widths: &[usize]) -> bool {
    match widths.first() {
        None => true,
        Some(&first) => widths.iter().all(|&w| w == first),
    }
}

/// Collect the widths of all top-level elements in an array.
fn collect_element_widths(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> Vec<usize> {
    let mut collector = ElementWidthCollector::new();

    for i in (open_index + 1)..close_index {
        collector.process_token(&tokens.tokens[i]);
    }

    collector.widths
}

/// State machine for collecting element widths from an array.
struct ElementWidthCollector {
    widths: Vec<usize>,
    depth: i32,
    current_width: usize,
    in_nested_element: bool,
}

impl ElementWidthCollector {
    fn new() -> Self {
        Self {
            widths: Vec::new(),
            depth: 0,
            current_width: 0,
            in_nested_element: false,
        }
    }

    fn process_token(&mut self, token: &TomlToken<'_>) {
        match token.kind {
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => self.enter_nested(token),
            TokenKind::ArrayClose | TokenKind::InlineTableClose => self.exit_nested(token),
            TokenKind::Scalar => self.handle_scalar(token),
            TokenKind::ValueSep if self.depth == 0 => self.handle_top_level_comma(),
            TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment => {}
            _ if self.depth > 0 => self.current_width += token.raw.len(),
            _ => {}
        }
    }

    fn enter_nested(&mut self, token: &TomlToken<'_>) {
        self.depth += 1;
        if self.depth == 1 {
            self.in_nested_element = true;
        }
        self.current_width += token.raw.len();
    }

    fn exit_nested(&mut self, token: &TomlToken<'_>) {
        self.current_width += token.raw.len();
        self.depth -= 1;
        if self.depth == 0 && self.in_nested_element {
            self.finish_nested_element();
        }
    }

    fn handle_scalar(&mut self, token: &TomlToken<'_>) {
        if self.depth == 0 {
            self.widths.push(token.raw.len());
        } else {
            self.current_width += token.raw.len();
        }
    }

    fn handle_top_level_comma(&mut self) {
        if self.in_nested_element {
            self.finish_nested_element();
        }
    }

    fn finish_nested_element(&mut self) {
        self.widths.push(self.current_width);
        self.current_width = 0;
        self.in_nested_element = false;
    }
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

/// Reflow array with horizontal grouping (comments act as line-enders).
///
/// Groups elements horizontally on each line. Standalone comments end their line,
/// with subsequent elements starting a new line.
fn reflow_grouped(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    tab_spaces: usize,
    nesting_depth: usize,
    array_width: usize,
) {
    // Detect standalone trailing comment BEFORE collapse (comment on its own line at the end)
    let has_standalone_trailing_comment =
        has_standalone_trailing_comment(tokens, open_index, close_index);

    // First collapse to normalize, then reflow with grouping
    let close = remove_newlines_and_indents(tokens, open_index, close_index);
    let close = remove_pre_comma_whitespace(tokens, open_index, close);
    normalize_comma_spacing(tokens, open_index, close);

    // Find new close after normalization
    let close = find_array_close(tokens, open_index).unwrap_or(close);

    // Now reflow with horizontal grouping
    let config = GroupingConfig {
        indent: make_indent(nesting_depth + 1, tab_spaces),
        close_indent: make_indent(nesting_depth, tab_spaces),
        array_width,
        tab_spaces,
        has_standalone_trailing_comment,
    };

    let insertions = collect_grouped_insertions(tokens, open_index, close, &config);

    apply_newline_insertions(tokens, insertions);
    remove_trailing_whitespace(tokens);
    tokens.trim_empty_whitespace();
}

/// Remove trailing whitespace (whitespace directly before newlines).
fn remove_trailing_whitespace(tokens: &mut TomlTokens<'_>) {
    let mut i = 0;
    while i + 1 < tokens.tokens.len() {
        if tokens.tokens[i].kind == TokenKind::Whitespace
            && tokens.tokens[i + 1].kind == TokenKind::Newline
        {
            // Clear this whitespace token (it's trailing)
            tokens.tokens[i].raw = Cow::Borrowed("");
        }
        i += 1;
    }
}

/// Check if the array has a standalone trailing comment (comment on its own line at the end).
fn has_standalone_trailing_comment(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> bool {
    find_trailing_comment_index(tokens, open_index, close_index)
        .map(|idx| is_on_own_line(tokens, idx, open_index))
        .unwrap_or(false)
}

/// Find the index of a trailing comment (last non-whitespace token before close bracket).
/// Returns None if there's no comment or if there's other content.
fn find_trailing_comment_index(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> Option<usize> {
    let idx = skip_backwards(tokens, close_index.saturating_sub(1), open_index, |kind| {
        matches!(kind, TokenKind::Whitespace | TokenKind::Newline)
    });

    if idx > open_index && tokens.tokens[idx].kind == TokenKind::Comment {
        Some(idx)
    } else {
        None
    }
}

/// Check if a token is on its own line (preceded by newline after skipping whitespace only).
fn is_on_own_line(tokens: &TomlTokens<'_>, index: usize, min_index: usize) -> bool {
    if index <= min_index {
        return false;
    }

    // Skip only whitespace (not newlines) to check if preceded by newline
    let check = skip_backwards(tokens, index - 1, min_index, |kind| {
        kind == TokenKind::Whitespace
    });

    check > min_index && tokens.tokens[check].kind == TokenKind::Newline
}

/// Skip tokens backwards while predicate matches.
fn skip_backwards(
    tokens: &TomlTokens<'_>,
    start: usize,
    min_index: usize,
    should_skip: impl Fn(TokenKind) -> bool,
) -> usize {
    let mut idx = start;
    while idx > min_index && should_skip(tokens.tokens[idx].kind) {
        idx -= 1;
    }
    idx
}

/// Configuration for grouped horizontal layout.
struct GroupingConfig {
    indent: String,
    close_indent: String,
    array_width: usize,
    tab_spaces: usize,
    has_standalone_trailing_comment: bool,
}

/// State for collecting grouped insertions.
struct GroupingState<'a> {
    insertions: Vec<(usize, String)>,
    current_line_width: usize,
    base_width: usize,
    indent: &'a str,
}

impl<'a> GroupingState<'a> {
    fn new(base_width: usize, indent: &'a str) -> Self {
        Self {
            insertions: Vec::new(),
            current_line_width: base_width + indent.len(),
            base_width,
            indent,
        }
    }

    fn insert_newline(&mut self, index: usize) {
        self.insertions.push((index, self.indent.to_owned()));
        self.current_line_width = self.base_width + self.indent.len();
    }

    fn update_width(&mut self, projected: usize) {
        self.current_line_width = projected;
    }
}

/// Collect insertion points for grouped horizontal layout.
fn collect_grouped_insertions(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    config: &GroupingConfig,
) -> Vec<(usize, String)> {
    let base_width = calculate_base_width(tokens, open_index, config.tab_spaces);
    let mut state = GroupingState::new(base_width, &config.indent);

    // Insert newline after open bracket
    state.insert_newline(open_index + 1);

    let mut local_depth = 0;

    for i in (open_index + 1)..close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        if local_depth != 0 {
            continue;
        }

        match kind {
            TokenKind::Comment => {
                handle_comment_insertion(tokens, i, close_index, &mut state);
            }
            TokenKind::ValueSep => {
                handle_comma_insertion(tokens, i, close_index, config, &mut state);
            }
            _ => {}
        }
    }

    // Newline before close bracket
    state
        .insertions
        .push((close_index, config.close_indent.clone()));
    state.insertions
}

/// Calculate base width from line start to array open bracket.
fn calculate_base_width(tokens: &TomlTokens<'_>, open_index: usize, tab_spaces: usize) -> usize {
    let line_start = find_line_start(tokens, open_index);
    tokens.tokens[line_start..=open_index]
        .iter()
        .map(|t| token_width(&t.raw, tab_spaces))
        .sum()
}

/// Handle insertion after a comment token.
fn handle_comment_insertion(
    tokens: &TomlTokens<'_>,
    comment_index: usize,
    close_index: usize,
    state: &mut GroupingState<'_>,
) {
    // Only insert newline after comments that have values following them
    let has_value_after = has_value_after_index(tokens, comment_index, close_index);
    if has_value_after && comment_index + 1 < close_index {
        state.insert_newline(comment_index + 1);
    }
}

/// Handle insertion after a comma token.
fn handle_comma_insertion(
    tokens: &TomlTokens<'_>,
    comma_index: usize,
    close_index: usize,
    config: &GroupingConfig,
    state: &mut GroupingState<'_>,
) {
    match peek_after_comma(tokens, comma_index, close_index, config.tab_spaces) {
        NextAfterComma::Element { width, index } => {
            let projected_width = state.current_line_width + 2 + width; // ", " + element
            if projected_width > config.array_width {
                state.insert_newline(index);
            } else {
                state.update_width(projected_width);
            }
        }
        NextAfterComma::TrailingComment if config.has_standalone_trailing_comment => {
            let comment_idx = skip_whitespace(tokens, comma_index + 1, close_index);
            state.insert_newline(comment_idx);
        }
        _ => {}
    }
}

/// Skip whitespace tokens and return the index of the next non-whitespace token.
fn skip_whitespace(tokens: &TomlTokens<'_>, start: usize, end: usize) -> usize {
    let mut idx = start;
    while idx < end && tokens.tokens[idx].kind == TokenKind::Whitespace {
        idx += 1;
    }
    idx
}

/// What comes after a comma in an array.
enum NextAfterComma {
    /// An element with the given width, starting at the given index
    Element { width: usize, index: usize },
    /// A trailing comment (no values after it)
    TrailingComment,
    /// Nothing (close bracket)
    Nothing,
}

/// Peek at what comes after a comma.
fn peek_after_comma(
    tokens: &TomlTokens<'_>,
    comma_index: usize,
    close_index: usize,
    tab_spaces: usize,
) -> NextAfterComma {
    let mut i = comma_index + 1;

    // Skip whitespace after comma
    while i < close_index && tokens.tokens[i].kind == TokenKind::Whitespace {
        i += 1;
    }

    if i >= close_index {
        return NextAfterComma::Nothing;
    }

    // Check if it's a comment with no values after
    if tokens.tokens[i].kind == TokenKind::Comment && !has_value_after_index(tokens, i, close_index)
    {
        return NextAfterComma::TrailingComment;
    }

    // Record the starting index of the element (after whitespace)
    let element_start = i;

    // Accumulate element width
    let mut width = 0;
    let mut local_depth = 0;

    while i < close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        match kind {
            TokenKind::ValueSep | TokenKind::Comment if local_depth == 0 => break,
            TokenKind::ArrayClose if local_depth == 0 => break,
            _ => {
                width += token_width(&tokens.tokens[i].raw, tab_spaces);
            }
        }
        i += 1;
    }

    if width > 0 {
        NextAfterComma::Element {
            width,
            index: element_start,
        }
    } else {
        NextAfterComma::Nothing
    }
}

/// Ensure array has a trailing comma after the last element.
///
/// Returns the updated close index (incremented by 1 if comma was inserted).
fn ensure_trailing_comma(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> usize {
    match find_last_value_needing_comma(tokens, open_index, close_index) {
        LastValueResult::AlreadyHasTrailingComma => close_index,
        LastValueResult::NeedsCommaAfter(idx) => {
            tokens.tokens.insert(idx + 1, TomlToken::VAL_SEP);
            close_index + 1
        }
        LastValueResult::Empty => close_index,
    }
}

/// Result of searching for the last value that needs a trailing comma.
enum LastValueResult {
    /// Array already has a trailing comma
    AlreadyHasTrailingComma,
    /// Last value found at index, needs comma after it
    NeedsCommaAfter(usize),
    /// Array is empty or has no values
    Empty,
}

/// Find the last value in an array that needs a trailing comma.
fn find_last_value_needing_comma(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> LastValueResult {
    let mut last_value_index = None;
    let mut local_depth = 0;

    for i in (open_index + 1)..close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        match classify_token_for_trailing_comma(kind, local_depth) {
            TrailingCommaAction::FoundValue => last_value_index = Some(i),
            TrailingCommaAction::CheckComma => {
                if is_trailing_comma(tokens, i, close_index) {
                    return LastValueResult::AlreadyHasTrailingComma;
                }
            }
            TrailingCommaAction::Skip => {}
        }
    }

    match last_value_index {
        Some(idx) => LastValueResult::NeedsCommaAfter(idx),
        None => LastValueResult::Empty,
    }
}

/// Calculate depth change for a token kind.
fn depth_delta(kind: TokenKind) -> i32 {
    match kind {
        TokenKind::ArrayOpen | TokenKind::InlineTableOpen => 1,
        TokenKind::ArrayClose | TokenKind::InlineTableClose => -1,
        _ => 0,
    }
}

/// Action to take when scanning for trailing comma.
enum TrailingCommaAction {
    FoundValue,
    CheckComma,
    Skip,
}

/// Classify a token for trailing comma detection.
fn classify_token_for_trailing_comma(kind: TokenKind, depth: i32) -> TrailingCommaAction {
    match kind {
        TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment => {
            TrailingCommaAction::Skip
        }
        TokenKind::ValueSep if depth == 0 => TrailingCommaAction::CheckComma,
        TokenKind::ArrayClose | TokenKind::InlineTableClose if depth == 0 => {
            TrailingCommaAction::FoundValue
        }
        _ if depth == 0 => TrailingCommaAction::FoundValue,
        _ => TrailingCommaAction::Skip,
    }
}

/// Clear whitespace immediately after commas to prepare for reformatting.
///
/// Preserves space before comments (space between comma and inline comment).
fn clear_post_comma_whitespace(tokens: &mut TomlTokens<'_>, open_index: usize, close_index: usize) {
    let indices_to_clear: Vec<usize> =
        find_clearable_post_comma_whitespace(tokens, open_index, close_index);
    for i in indices_to_clear {
        tokens.tokens[i] = TomlToken::EMPTY;
    }
}

/// Find indices of whitespace tokens after commas that should be cleared.
///
/// Preserves whitespace before comments (inline comment spacing).
fn find_clearable_post_comma_whitespace(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> Vec<usize> {
    let mut result = Vec::new();
    let mut local_depth = 0;

    for i in (open_index + 1)..close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        if kind == TokenKind::ValueSep && local_depth == 0 {
            if let Some(ws_index) = clearable_whitespace_after(tokens, i, close_index) {
                result.push(ws_index);
            }
        }
    }

    result
}

/// Check if whitespace after a comma should be cleared.
///
/// Returns the whitespace index if it should be cleared, None otherwise.
/// Preserves whitespace before comments.
fn clearable_whitespace_after(
    tokens: &TomlTokens<'_>,
    comma_index: usize,
    close_index: usize,
) -> Option<usize> {
    let ws_index = comma_index + 1;
    if ws_index >= close_index || tokens.tokens[ws_index].kind != TokenKind::Whitespace {
        return None;
    }

    // Check what follows the whitespace
    let next_kind = tokens.tokens[(ws_index + 1)..close_index]
        .iter()
        .find(|t| t.kind != TokenKind::Whitespace)
        .map(|t| t.kind);

    // Preserve space before comments
    if next_kind == Some(TokenKind::Comment) {
        return None;
    }

    Some(ws_index)
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
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        if let Some(insert_index) = insertion_point_for_token(tokens, i, close_index, local_depth) {
            insertions.push((insert_index, indent));
        }
    }

    insertions.push((close_index, close_indent));
    insertions
}

/// Determine if a token requires a newline insertion after it.
///
/// Returns the index where newline should be inserted, or None.
fn insertion_point_for_token(
    tokens: &TomlTokens<'_>,
    index: usize,
    close_index: usize,
    depth: i32,
) -> Option<usize> {
    if depth != 0 {
        return None;
    }

    match tokens.tokens[index].kind {
        TokenKind::ValueSep => insertion_after_comma(tokens, index, close_index),
        TokenKind::Comment => insertion_after_comment(tokens, index, close_index),
        _ => None,
    }
}

/// Check if newline should be inserted after a comma.
///
/// Skips trailing commas and commas followed by inline comments.
fn insertion_after_comma(
    tokens: &TomlTokens<'_>,
    comma_index: usize,
    close_index: usize,
) -> Option<usize> {
    if is_trailing_comma(tokens, comma_index, close_index) {
        return None;
    }

    // Skip if followed by inline comment (newline comes after comment instead)
    if is_followed_by_comment(tokens, comma_index, close_index) {
        return None;
    }

    Some(comma_index + 1)
}

/// Check if newline should be inserted after a comment.
///
/// Only inserts if more elements follow (close bracket gets its own newline).
fn insertion_after_comment(
    tokens: &TomlTokens<'_>,
    comment_index: usize,
    close_index: usize,
) -> Option<usize> {
    if has_value_after_index(tokens, comment_index, close_index) {
        Some(comment_index + 1)
    } else {
        None
    }
}

/// Check if a comma is followed by a comment (skipping whitespace).
fn is_followed_by_comment(tokens: &TomlTokens<'_>, comma_index: usize, close_index: usize) -> bool {
    tokens.tokens[(comma_index + 1)..close_index]
        .iter()
        .find(|t| t.kind != TokenKind::Whitespace)
        .map(|t| t.kind == TokenKind::Comment)
        .unwrap_or(false)
}

/// Apply newline + indent insertions in reverse order to maintain indices.
fn apply_newline_insertions<S: AsRef<str>>(
    tokens: &mut TomlTokens<'_>,
    insertions: Vec<(usize, S)>,
) {
    for (index, indent) in insertions.into_iter().rev() {
        let indent = indent.as_ref();
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
        .all(|t| {
            matches!(
                t.kind,
                TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment
            )
        })
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
    // Check comment position - only collapse if no comments or comment only on last element
    match comment_position(tokens, open_index, close_index) {
        CommentPosition::None | CommentPosition::LastElementOnly => {}
        CommentPosition::NonLastElement | CommentPosition::BeforeClose => return false,
    }

    // Calculate collapsed width (including any trailing comment)
    let collapsed_width = calculate_collapsed_width(tokens, open_index, close_index, tab_spaces);

    collapsed_width <= array_width
}

/// Position of comments within an array.
enum CommentPosition {
    /// No comments in the array
    None,
    /// Comment only on the last element (can collapse)
    LastElementOnly,
    /// Comment on a non-last element (cannot collapse)
    NonLastElement,
    /// Comment before the closing bracket (cannot collapse)
    BeforeClose,
}

/// State for tracking comment positions during array iteration.
struct CommentState {
    last_value_index: Option<usize>,
    has_trailing_comment: bool,
    has_non_last_comment: bool,
}

impl CommentState {
    fn new() -> Self {
        Self {
            last_value_index: None,
            has_trailing_comment: false,
            has_non_last_comment: false,
        }
    }

    fn record_value(&mut self, index: usize) {
        if self.has_trailing_comment {
            self.has_non_last_comment = true;
        }
        self.last_value_index = Some(index);
        self.has_trailing_comment = false;
    }

    fn into_position(self) -> CommentPosition {
        if self.has_non_last_comment {
            CommentPosition::NonLastElement
        } else if self.has_trailing_comment {
            CommentPosition::LastElementOnly
        } else {
            CommentPosition::None
        }
    }
}

/// Determine where comments are positioned in the array.
fn comment_position(
    tokens: &TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
) -> CommentPosition {
    let mut state = CommentState::new();
    let mut local_depth = 0;

    for i in (open_index + 1)..close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        match kind {
            TokenKind::Comment => {
                if let Some(result) = handle_comment(tokens, i, close_index, &mut state) {
                    return result;
                }
            }
            TokenKind::ArrayClose | TokenKind::InlineTableClose if local_depth == 0 => {
                state.record_value(i);
            }
            TokenKind::Whitespace | TokenKind::Newline | TokenKind::ValueSep => {}
            _ if local_depth == 0 => {
                state.record_value(i);
            }
            _ => {}
        }
    }

    state.into_position()
}

/// Handle a comment token and update state.
///
/// Returns `Some(CommentPosition)` for early exit, `None` to continue iteration.
fn handle_comment(
    tokens: &TomlTokens<'_>,
    comment_index: usize,
    close_index: usize,
    state: &mut CommentState,
) -> Option<CommentPosition> {
    let Some(last_idx) = state.last_value_index else {
        state.has_non_last_comment = true;
        return None;
    };

    if is_same_line(tokens, last_idx, comment_index) {
        state.has_trailing_comment = true;
        return None;
    }

    // Comment is after a newline - check if there are more values after
    if has_value_after_index(tokens, comment_index, close_index) {
        state.has_non_last_comment = true;
        None
    } else {
        Some(CommentPosition::BeforeClose)
    }
}

/// Check if two indices are on the same line (no newlines between them).
fn is_same_line(tokens: &TomlTokens<'_>, from: usize, to: usize) -> bool {
    !tokens.tokens[from..to]
        .iter()
        .any(|t| t.kind == TokenKind::Newline)
}

/// Check if there's a value after the given index.
fn has_value_after_index(tokens: &TomlTokens<'_>, start: usize, close_index: usize) -> bool {
    let mut local_depth = 0;
    for i in (start + 1)..close_index {
        let kind = tokens.tokens[i].kind;
        local_depth += depth_delta(kind);

        match kind {
            TokenKind::Whitespace
            | TokenKind::Newline
            | TokenKind::Comment
            | TokenKind::ValueSep => {}
            TokenKind::ArrayClose | TokenKind::InlineTableClose if local_depth < 0 => {}
            _ if local_depth == 0 => return true,
            _ => {}
        }
    }
    false
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

/// Collapse elements to horizontal but keep closing bracket on new line.
///
/// Used for arrays with a trailing comment on the last element.
fn collapse_with_trailing_comment(
    tokens: &mut TomlTokens<'_>,
    open_index: usize,
    close_index: usize,
    nesting_depth: usize,
    tab_spaces: usize,
) {
    // First, collapse all elements (including removing the trailing comma's newline)
    let close = remove_newlines_and_indents(tokens, open_index, close_index);

    // Remove whitespace before commas (but keep trailing comma)
    let close = remove_pre_comma_whitespace(tokens, open_index, close);

    // Normalize comma spacing
    normalize_comma_spacing(tokens, open_index, close);

    // Find the new close index
    let new_close = find_array_close(tokens, open_index).unwrap_or(close);

    let indent = make_indent(nesting_depth + 1, tab_spaces);
    let close_indent = make_indent(nesting_depth, tab_spaces);

    // Insert newline + indent after opening bracket
    let insertions = vec![(open_index + 1, indent), (new_close, close_indent)];

    apply_newline_insertions(tokens, insertions);
}

/// Remove whitespace before commas (but NOT the trailing comma itself).
fn remove_pre_comma_whitespace(
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
        i += 1;
    }

    close
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
    fn valid(input: &str, max_width: usize, element_threshold: usize, expected: impl IntoData) {
        let mut tokens = TomlTokens::parse(input);
        super::reflow_arrays(
            &mut tokens,
            max_width,
            element_threshold,
            DEFAULT_TAB_SPACES,
        );
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
            str![[r#"
x = ['''
literal
''']

"#]],
        );
    }

    #[test]
    fn dotted_key_width_included() {
        // "foo.bar.baz = [\"a\", \"b\"]" = 24 chars
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            23,
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
            str![[r#"
x = [
    "a", # comment
    "b",
]

"#]],
        );
    }

    #[test]
    fn mixed_style_with_comment_normalized() {
        // Mixed-style arrays with comments are normalized with horizontal grouping.
        // Comment acts as line-ender, elements after continue on next line.
        valid(
            r#"x = ["a", "b", # comment
    "c",
]
"#,
            80,
            10,
            str![[r#"
x = [
    "a", "b", # comment
    "c",
]

"#]],
        );
    }

    #[test]
    fn grouped_elements_with_comments_normalized() {
        // Mixed-width arrays with comments: rustfmt uses one element per line.
        // Horizontal grouping only applies when all elements have uniform width.
        valid(
            r#"deps = [
    "a", "b", "c",
    "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", # comment about this group
    "x", "y", "z", # fits
]
"#,
            60,
            10,
            str![[r#"
deps = [
    "a",
    "b",
    "c",
    "aaaaaaaaaaaa",
    "bbbbbbbbbbbb",
    "cccccccccccc", # comment about this group
    "x",
    "y",
    "z", # fits
]

"#]],
        );
    }

    #[test]
    fn standalone_comment_groups_horizontally() {
        // Elements before a standalone comment are grouped on the same line as the comment.
        // Elements after the comment start a new line.
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
            10,
            str![[r#"
deps = [
    "a", "b", # comment about elements below
    "c", "d",
]

"#]],
        );
    }

    #[test]
    fn comment_on_last_element_collapses() {
        // Comment only on last element allows horizontal grouping.
        // Elements on new line after bracket, close bracket on new line.
        valid(
            r#"x = [
    "a",
    "b", # comment
]
"#,
            80,
            10,
            str![[r#"
x = [
    "a", "b", # comment
]

"#]],
        );
    }

    #[test]
    fn comment_before_close_stays_vertical() {
        // Trailing comment (before close bracket) stays on its own line.
        // Elements are grouped horizontally.
        valid(
            r#"x = [
    "a",
    "b",
    # trailing comment
]
"#,
            80,
            10,
            str![[r#"
x = [
    "a", "b",
    # trailing comment
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
        valid("x = [\t1]\n", 11, 10, "x = [\t1]\n");
    }

    #[test]
    fn tabs_in_array_cause_reflow_at_boundary() {
        // "x = [\t1]" = 11 display columns
        // At max_width=10: should reflow
        // Note: tab inside content is preserved
        valid("x = [\t1]\n", 10, 10, "x = [\n    \t1,\n]\n");
    }

    #[test]
    fn tabs_between_elements_normalized_on_collapse() {
        // "x = [1, 2]" = 10 columns < 40
        valid(
            "x = [\n\t1,\n\t2,\n]\n",
            40,
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            10,
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
            20,
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
            10,
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
            10,
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
            10,
            str![[r#"
x = [   ]

"#]],
        );
    }

    #[test]
    fn element_width_threshold_triggers_reflow() {
        valid(
            r#"deps = ["short", "very-long-element-name"]"#,
            100,
            15,
            str![[r#"
deps = [
    "short",
    "very-long-element-name",
]
"#]],
        );
    }
}
