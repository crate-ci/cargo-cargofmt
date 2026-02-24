mod table;
mod tokens;

pub use table::Table;
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

    pub fn from_index(i: usize) -> Self {
        Self { i }
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

    pub fn prev_index(&mut self, _tokens: &TomlTokens<'_>) -> Option<usize> {
        if self.i > 0 {
            self.i -= 1;
            Some(self.i)
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn next_index_iterates_forward() {
        let tokens = TomlTokens::parse("a = 1");
        let mut indices = TokenIndices::new();
        assert_eq!(indices.next_index(&tokens), Some(0));
        assert_eq!(indices.next_index(&tokens), Some(1));
        assert_eq!(indices.next_index(&tokens), Some(2));
    }

    #[test]
    fn next_index_returns_none_at_end() {
        let tokens = TomlTokens::parse("a = 1");
        let mut indices = TokenIndices::new();
        // Exhaust all indices
        while indices.next_index(&tokens).is_some() {}
        // Should return None when exhausted
        assert_eq!(indices.next_index(&tokens), None);
        assert_eq!(indices.next_index(&tokens), None);
    }

    #[test]
    fn set_next_index_jumps_position() {
        let tokens = TomlTokens::parse("a = 1");
        let mut indices = TokenIndices::new();
        indices.set_next_index(2);
        assert_eq!(indices.next_index(&tokens), Some(2));
        assert_eq!(indices.next_index(&tokens), Some(3));
    }

    #[test]
    fn from_index_begins_at_position() {
        let tokens = TomlTokens::parse("a = 1");
        let mut indices = TokenIndices::from_index(2);
        assert_eq!(indices.next_index(&tokens), Some(2));
        assert_eq!(indices.next_index(&tokens), Some(3));
    }

    #[test]
    fn prev_index_from_start() {
        let tokens = TomlTokens::parse("key = 1");
        let mut indices = TokenIndices::new();
        // At position 0, prev_index should return None
        assert_eq!(indices.prev_index(&tokens), None);
    }

    #[test]
    fn prev_index_after_next() {
        let tokens = TomlTokens::parse("key = 1");
        let mut indices = TokenIndices::new();
        // Advance forward
        indices.next_index(&tokens);
        indices.next_index(&tokens);
        // Now at position 2, prev_index should return Some(1)
        assert_eq!(indices.prev_index(&tokens), Some(1));
        assert_eq!(indices.prev_index(&tokens), Some(0));
        assert_eq!(indices.prev_index(&tokens), None);
    }

    #[test]
    fn prev_index_iteration() {
        let tokens = TomlTokens::parse("[a.b]");
        let mut indices = TokenIndices::new();
        // Move to end
        indices.set_next_index(tokens.len());
        // Iterate backwards collecting all indices
        let mut reversed = Vec::new();
        while let Some(i) = indices.prev_index(&tokens) {
            reversed.push(i);
        }
        let expected: Vec<usize> = (0..tokens.len()).rev().collect();
        assert_eq!(reversed, expected);
    }
}
