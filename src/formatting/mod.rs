mod generated;
mod newline_style;
mod trailing_spaces;

pub use generated::is_generated_file;
pub use newline_style::apply_newline_style;
pub use trailing_spaces::trim_trailing_spaces;
