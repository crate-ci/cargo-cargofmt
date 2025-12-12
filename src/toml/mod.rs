mod tokens;

pub use tokens::Encoding;
pub use tokens::ScalarKind;
pub use tokens::TokenKind;
pub use tokens::TomlToken;
pub use tokens::TomlTokens;

pub struct TokenIndices {
    i: usize,
}

impl TokenIndices {
    pub fn new() -> Self {
        Self { i: 0 }
    }

    pub fn next_index(&mut self, tokens: &TomlTokens<'_>) -> Option<usize> {
        if self.i < tokens.len() {
            let i = self.i;
            self.i += 1;
            Some(i)
        } else {
            None
        }
    }

    pub fn set_next_index(&mut self, i: usize) {
        self.i = i;
    }

    pub fn rev(&self) -> impl Iterator<Item = usize> {
        (0..self.i).rev()
    }
}

impl Default for TokenIndices {
    fn default() -> Self {
        Self::new()
    }
}
