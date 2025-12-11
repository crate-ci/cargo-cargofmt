#[derive(Copy, Clone, Debug, serde::Deserialize)]
pub enum SeparatorTactic {
    Always,
    Never,
    Vertical,
}

impl SeparatorTactic {
    pub fn from_bool(b: bool) -> Self {
        if b {
            Self::Always
        } else {
            Self::Never
        }
    }
}
