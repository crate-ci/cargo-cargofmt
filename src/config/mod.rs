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
    pub hard_tabs: bool,
    pub tab_spaces: usize,
    pub max_width: usize,
    pub array_width: Option<usize>,
    pub short_array_element_width_threshold: usize,
    pub use_small_heuristics: options::UseSmallHeuristics,
}

impl Config {
    /// Returns the effective array width.
    ///
    /// If `array_width` is explicitly set, returns that value.
    /// Otherwise, calculates based on `use_small_heuristics`:
    /// - `Default`: 60% of `max_width`
    /// - `Off`: 0 (always vertical)
    /// - `Max`: `max_width`
    pub fn array_width(&self) -> usize {
        self.array_width
            .unwrap_or_else(|| self.heuristic_width(0.6))
    }

    fn heuristic_width(&self, percent: f64) -> usize {
        match self.use_small_heuristics {
            options::UseSmallHeuristics::Default => (self.max_width as f64 * percent) as usize,
            options::UseSmallHeuristics::Off => 0,
            options::UseSmallHeuristics::Max => self.max_width,
        }
    }
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
            hard_tabs: false,
            tab_spaces: 4,
            max_width: 100,
            array_width: None,
            short_array_element_width_threshold: 10,
            use_small_heuristics: options::UseSmallHeuristics::default(),
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn max_width() {
        assert_eq!(Config::default().max_width, 100);

        let config: Config = toml::de::from_str("max_width = 80").unwrap();
        assert_eq!(config.max_width, 80);
    }

    #[test]
    fn array_width_uses_heuristics() {
        // Default: 60% of max_width
        let config = Config::default();
        assert_eq!(config.array_width, None);
        assert_eq!(config.array_width(), 60); // 60% of 100

        let config: Config = toml::de::from_str("max_width = 80").unwrap();
        assert_eq!(config.array_width(), 48); // 60% of 80

        // Explicit array_width overrides heuristics
        let config: Config = toml::de::from_str("max_width = 100\narray_width = 70").unwrap();
        assert_eq!(config.array_width(), 70);
    }

    #[test]
    fn short_array_threshold_config() {
        let config = Config::default();
        assert_eq!(config.short_array_element_width_threshold, 10);

        let config: Config =
            toml::de::from_str("short_array_element_width_threshold = 20").unwrap();
        assert_eq!(config.short_array_element_width_threshold, 20);
    }

    #[test]
    fn use_small_heuristics_max() {
        let config: Config =
            toml::de::from_str("max_width = 100\nuse_small_heuristics = \"Max\"").unwrap();
        assert_eq!(config.array_width(), 100); // equals max_width
    }

    #[test]
    fn use_small_heuristics_off() {
        let config: Config =
            toml::de::from_str("max_width = 100\nuse_small_heuristics = \"Off\"").unwrap();
        assert_eq!(config.array_width(), 0); // always vertical
    }
}
