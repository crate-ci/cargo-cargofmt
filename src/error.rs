use crate::formatting;

pub enum FormattingError {
    /// A kind of error  
    LineOverflow {
        line: usize,
        width: usize,
        max_width: usize,
    },
}

impl FormattingError {
    /// Render the error
    pub fn display(&self, path: &str) -> String {
        match self {
            FormattingError::LineOverflow {
                line,
                width,
                max_width,
            } => format!("warning: {path}:{line} line exceeds max_width ({width} > {max_width})",),
        }
    }
}

pub fn check_errors(
    tokens: &crate::toml::TomlTokens<'_>,
    errors: &mut Vec<FormattingError>,
    error_on_line_overflow: bool,
    max_width: usize,
    tab_spaces: usize,
) {
    if error_on_line_overflow {
        let overflows = formatting::check_line_overflow(tokens, max_width, tab_spaces);
        errors.extend(
            overflows
                .into_iter()
                .map(|(line, width)| FormattingError::LineOverflow {
                    line,
                    width,
                    max_width,
                }),
        );
    }

    // unformatted error can be added
}
