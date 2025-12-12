mod blank_lines;
mod generated;
mod indent;
mod newline_style;
mod space_separators;
mod string;
mod trailing_comma;
mod trailing_spaces;

pub use blank_lines::constrain_blank_lines;
pub use generated::is_generated_file;
pub use indent::normalize_indent;
pub use newline_style::apply_newline_style;
pub use space_separators::normalize_space_separators;
pub use string::normalize_strings;
pub use trailing_comma::adjust_trailing_comma;
pub use trailing_spaces::trim_trailing_spaces;
