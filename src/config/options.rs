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
