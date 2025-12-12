use std::borrow::Cow;

use crate::toml::TokenKind;
use crate::toml::TomlToken;

#[tracing::instrument]
pub fn normalize_indent(
    tokens: &mut crate::toml::TomlTokens<'_>,
    hard_tabs: bool,
    tab_spaces: usize,
) {
    let mut depth = 0;
    let mut indices = crate::toml::TokenIndices::new();
    let mut buffer = PaddingBuffer::new(hard_tabs, tab_spaces);
    while let Some(i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {}
            TokenKind::StdTableClose | TokenKind::ArrayTableClose => {}
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => {
                depth += 1;
            }
            TokenKind::ArrayClose | TokenKind::InlineTableClose => {
                depth -= 1;
            }
            TokenKind::SimpleKey => {}
            TokenKind::KeySep => {}
            TokenKind::KeyValSep => {}
            TokenKind::Scalar => {}
            TokenKind::ValueSep => {}
            TokenKind::Whitespace => {}
            TokenKind::Comment => {}
            TokenKind::Newline => {
                let next_i = i + 1;
                if let Some(next) = tokens.tokens.get_mut(next_i) {
                    match (next.kind, depth) {
                        (TokenKind::Newline, _) => {}
                        (TokenKind::Whitespace, 0) => {
                            *next = TomlToken::EMPTY;
                        }
                        (TokenKind::Whitespace, _) => {
                            let close_count = close_count(tokens, next_i);
                            let ws = buffer.whitespace(depth - close_count);
                            let mut token = TomlToken::EMPTY;
                            token.raw = Cow::Owned(ws.to_owned());
                            tokens.tokens[next_i] = token;
                        }
                        (_, 0) => {}
                        (_, _) => {
                            let close_count = close_count(tokens, next_i);
                            let ws = buffer.whitespace(depth - close_count);
                            let mut token = TomlToken::EMPTY;
                            token.raw = Cow::Owned(ws.to_owned());
                            tokens.tokens.insert(next_i, token);
                        }
                    }
                }
            }
            TokenKind::Error => {}
        }
    }
    tokens.trim_empty_whitespace();
}

struct PaddingBuffer {
    buffer: String,
    c: &'static str,
    count_per_indent: usize,
}

impl PaddingBuffer {
    fn new(hard_tabs: bool, tab_spaces: usize) -> Self {
        let (count_per_indent, c) = if hard_tabs {
            (1, "\t")
        } else {
            (tab_spaces, " ")
        };
        Self {
            buffer: Default::default(),
            c,
            count_per_indent,
        }
    }

    fn whitespace(&mut self, depth: usize) -> &str {
        let count = depth * self.count_per_indent;

        self.buffer.truncate(count);
        if let Some(add) = count.checked_sub(self.buffer.len()) {
            self.buffer.reserve(add);
            for _ in 0..add {
                self.buffer.push_str(self.c);
            }
        }

        &self.buffer
    }
}

fn close_count(tokens: &crate::toml::TomlTokens<'_>, i: usize) -> usize {
    let token_line_count = tokens.tokens[i..]
        .iter()
        .take_while(|t| {
            !matches!(
                t.kind,
                TokenKind::Newline
                    | TokenKind::Comment
                    | TokenKind::ArrayOpen
                    | TokenKind::InlineTableOpen
            )
        })
        .count();
    let end = i + token_line_count + 1;
    tokens.tokens[i..end]
        .iter()
        .filter(|t| matches!(t.kind, TokenKind::ArrayClose | TokenKind::InlineTableClose))
        .count()
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, hard_tabs: bool, tab_spaces: usize, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::normalize_indent(&mut tokens, hard_tabs, tab_spaces);
        let actual = tokens.to_string();

        assert_data_eq!(&actual, expected);

        let (_, errors) = toml::de::DeTable::parse_recoverable(&actual);
        if !errors.is_empty() {
            use std::fmt::Write as _;
            let mut result = String::new();
            writeln!(&mut result, "---").unwrap();
            for error in errors {
                writeln!(&mut result, "{error}").unwrap();
                writeln!(&mut result, "---").unwrap();
            }
            panic!("failed to parse\n---\n{actual}\n{result}");
        }
    }

    #[test]
    fn empty_tabs() {
        valid("", true, 10, str![]);
    }

    #[test]
    fn empty_spaces() {
        valid("", false, 10, str![]);
    }

    #[test]
    fn cleanup_tabs() {
        valid(
            "
  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11
",
            true,
            10,
            str![[r#"

a = 5

# Hello

[b]
a = 10
b = [
	1,
	2,
	3,
]
c = [
	[
		1,
		2,
		3,
	]
]
d = [[
		1,
		2,
		3,
]]

[e]
f = 10

g = 11

"#]],
        );
    }

    #[test]
    fn cleanup_spaces() {
        valid(
            "
  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11
",
            false,
            10,
            str![[r#"

a = 5

# Hello

[b]
a = 10
b = [
          1,
          2,
          3,
]
c = [
          [
                    1,
                    2,
                    3,
          ]
]
d = [[
                    1,
                    2,
                    3,
]]

[e]
f = 10

g = 11

"#]],
        );
    }
}
