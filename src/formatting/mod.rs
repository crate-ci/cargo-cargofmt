mod blank_lines;
mod generated;
mod newline_style;
mod space_separators;
mod trailing_spaces;

pub use blank_lines::constrain_blank_lines;
pub use generated::is_generated_file;
pub use newline_style::apply_newline_style;
pub use space_separators::normalize_space_separators;
pub use trailing_spaces::trim_trailing_spaces;
