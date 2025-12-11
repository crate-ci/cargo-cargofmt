use std::borrow::Cow;

pub use toml_parser::decoder::Encoding;
pub use toml_parser::decoder::ScalarKind;
pub use toml_parser::parser::EventKind as TokenKind;

#[derive(Debug)]
pub struct TomlTokens<'i> {
    pub tokens: Vec<TomlToken<'i>>,
    input_len: usize,
}

impl<'i> TomlTokens<'i> {
    pub fn parse(input: &'i str) -> Self {
        let source = toml_parser::Source::new(input);
        let tokens = source.lex().into_vec();

        let mut events = Vec::with_capacity(tokens.len());
        toml_parser::parser::parse_document(&tokens, &mut events, &mut ());

        let tokens = events
            .into_iter()
            .map(|e| {
                let raw = source.get(e).expect("already validated");
                let mut decoded = None;
                let mut scalar = None;
                match e.kind() {
                    TokenKind::SimpleKey => {
                        let mut d = Cow::Borrowed("");
                        raw.decode_key(&mut d, &mut ());
                        decoded = Some(d);
                    }
                    TokenKind::Scalar => {
                        let mut d = Cow::Borrowed("");
                        let s = raw.decode_scalar(&mut d, &mut ());
                        if matches!(s, ScalarKind::String) {
                            decoded = Some(d);
                        } else {
                            scalar = Some(s);
                        }
                    }
                    _ => {}
                }
                TomlToken {
                    kind: e.kind(),
                    encoding: e.encoding(),
                    decoded,
                    scalar,
                    raw: Cow::Borrowed(raw.as_str()),
                }
            })
            .collect();

        Self {
            tokens,
            input_len: input.len(),
        }
    }

    pub fn indices(&self) -> impl Iterator<Item = usize> {
        0..self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn trim_empty_whitespace(&mut self) {
        self.tokens
            .retain(|t| !(matches!(t.kind, TokenKind::Whitespace) && t.raw.is_empty()));
    }

    #[allow(clippy::inherent_to_string_shadow_display)]
    pub fn to_string(&self) -> String {
        use std::fmt::Write as _;

        let mut result = String::with_capacity(self.input_len);
        write!(&mut result, "{self}").unwrap();
        result
    }
}

impl std::fmt::Display for TomlTokens<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for token in &self.tokens {
            token.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TomlToken<'i> {
    pub kind: TokenKind,
    pub encoding: Option<Encoding>,
    pub decoded: Option<Cow<'i, str>>,
    pub scalar: Option<ScalarKind>,
    pub raw: Cow<'i, str>,
}

impl TomlToken<'_> {
    pub const EMPTY: Self = Self {
        kind: TokenKind::Whitespace,
        encoding: None,
        decoded: None,
        scalar: None,
        raw: Cow::Borrowed(""),
    };
    pub const SPACE: Self = Self {
        kind: TokenKind::Whitespace,
        encoding: None,
        decoded: None,
        scalar: None,
        raw: Cow::Borrowed(" "),
    };
    pub const NL: Self = Self {
        kind: TokenKind::Newline,
        encoding: None,
        decoded: None,
        scalar: None,
        raw: Cow::Borrowed("\n"), // assuming operating on normalized newlines
    };
}

impl std::fmt::Display for TomlToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.raw.fmt(f)
    }
}
