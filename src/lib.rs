//! > Cargo file formatter

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(clippy::print_stderr)]
#![warn(clippy::print_stdout)]

pub mod config;
pub mod formatting;
pub mod toml;

pub fn fmt_manifest(raw_input_text: &str, config: config::Config) -> Option<String> {
    if config.disable_all_formatting {
        return None;
    }

    if !config.format_generated_files
        && formatting::is_generated_file(raw_input_text, config.generated_marker_line_search_limit)
    {
        return None;
    }

    let mut input = raw_input_text.to_owned();

    // Normalize for easier manipulation
    formatting::apply_newline_style(
        config::options::NewlineStyle::Unix,
        &mut input,
        raw_input_text,
    );

    let mut tokens = toml::TomlTokens::parse(&input);

    formatting::normalize_strings(&mut tokens);
    formatting::normalize_datetime_separators(&mut tokens);
    formatting::trim_trailing_spaces(&mut tokens);
    formatting::normalize_space_separators(&mut tokens);
    formatting::reflow_arrays(&mut tokens, config.max_width, config.tab_spaces);
    formatting::constrain_blank_lines(
        &mut tokens,
        config.blank_lines_lower_bound,
        config.blank_lines_upper_bound,
    );
    formatting::adjust_trailing_comma(&mut tokens, config.trailing_comma);
    formatting::normalize_indent(&mut tokens, config.hard_tabs, config.tab_spaces);

    let mut formatted = tokens.to_string();

    formatting::apply_newline_style(config.newline_style, &mut formatted, raw_input_text);

    Some(formatted)
}

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

#[cfg(test)]
mod test {
    use super::*;

    /// Integration test exercising the full formatting pipeline.
    ///
    /// Input triggers:
    /// - `normalize_space_separators`: `name="test"` → `name = "test"`
    /// - `trim_trailing_spaces`: trailing spaces after value
    /// - `normalize_datetime_separators`: space → `T` separator
    /// - `constrain_blank_lines`: collapses multiple blank lines
    /// - `reflow_arrays`: array exceeding `max_width` (currently stub)
    /// - `adjust_trailing_comma`: adds comma to vertical arrays
    #[test]
    fn fmt_manifest_integration() {
        // concat! used for input to make trailing spaces visible
        // Currently: no reflow (stub does nothing)
        let input = concat!(
            r#"name="test""#,
            "   \n", // trailing spaces
            "date = 2024-01-01 12:00:00\n",
            "\n",
            "\n",
            r#"deps = ["foo", "bar", "baz", "qux"]"#,
            "\n",
        );
        let expected = r#"name = "test"
date = 2024-01-01T12:00:00

deps = ["foo", "bar", "baz", "qux"]
"#;
        let config = config::Config {
            max_width: 30,
            ..Default::default()
        };
        assert_eq!(fmt_manifest(input, config).unwrap(), expected);
    }
}
