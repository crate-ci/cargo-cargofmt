use std::io;
use std::path::Path;
use std::path::PathBuf;

pub mod lists;
pub mod options;

#[derive(serde::Deserialize)]
#[serde(default)]
pub struct Config {
    pub disable_all_formatting: bool,
    pub newline_style: options::NewlineStyle,
    pub format_generated_files: bool,
    pub generated_marker_line_search_limit: usize,
    pub blank_lines_lower_bound: usize,
    pub blank_lines_upper_bound: usize,
    pub trailing_comma: lists::SeparatorTactic,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            disable_all_formatting: false,
            newline_style: options::NewlineStyle::default(),
            format_generated_files: false,
            generated_marker_line_search_limit: 5,
            blank_lines_lower_bound: 0,
            blank_lines_upper_bound: 1,
            trailing_comma: lists::SeparatorTactic::Vertical,
        }
    }
}

#[tracing::instrument]
pub fn load_config(search_start: &Path) -> Result<Config, io::Error> {
    let Some(path) = find_config(search_start) else {
        return Ok(Config::default());
    };

    let content = std::fs::read_to_string(&path)?;
    let config = toml::de::from_str(&content).map_err(io::Error::other)?;
    Ok(config)
}

fn find_config(mut path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        path = path.parent()?;
    }

    loop {
        let mut config_path = path.join("");
        for name in CONFIG_FILE_NAMES {
            config_path.set_file_name(name);
            if config_path.is_file() {
                return Some(config_path);
            }
        }

        path = path.parent()?;
    }
}

const CONFIG_FILE_NAMES: [&str; 2] = [".rustfmt.toml", "rustfmt.toml"];
