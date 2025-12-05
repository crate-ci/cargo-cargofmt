/// Returns `true` if the given span is a part of generated files.
pub fn is_generated_file(
    original_snippet: &str,
    generated_marker_line_search_limit: usize,
) -> bool {
    original_snippet
        .lines()
        // looking for marker only in the beginning of the file
        .take(generated_marker_line_search_limit)
        .any(|line| line.contains("@generated"))
}
