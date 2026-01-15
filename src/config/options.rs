#[derive(Copy, Clone, Default, Debug, serde::Deserialize)]
pub enum NewlineStyle {
    /// Auto-detect based on the raw source input.
    #[default]
    Auto,
    /// Force CRLF (`\r\n`).
    Windows,
    /// Force CR (`\n`).
    Unix,
    /// `\r\n` in Windows, `\n` on other platforms.
    Native,
}

/// Controls how width heuristics are calculated for formatting decisions.
#[derive(Copy, Clone, Default, Debug, serde::Deserialize)]
pub enum UseSmallHeuristics {
    /// Calculate widths as percentage of `max_width` (e.g., `array_width` = 60%).
    #[default]
    Default,
    /// Disable width heuristics (always use vertical layout).
    Off,
    /// Use `max_width` for all width settings.
    Max,
}
