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
    formatting::remove_unused_parent_tables(&mut tokens);
    formatting::trim_trailing_spaces(&mut tokens);
    formatting::normalize_space_separators(&mut tokens);
    formatting::reflow_arrays(
        &mut tokens,
        config.max_width,     // hard line-length limit  (collapse threshold)
        config.array_width(), // array-content soft limit (expand threshold)
        config.tab_spaces,
    );
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
