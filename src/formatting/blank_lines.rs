use crate::toml::TokenKind;
use crate::toml::TomlToken;

/// Assumptions:
/// - newlines normalized
/// - trailing spaces trimmed
#[tracing::instrument]
pub fn constrain_blank_lines(tokens: &mut crate::toml::TomlTokens<'_>, min: usize, max: usize) {
    let mut depth = 0;
    let mut indices = crate::toml::TokenIndices::new();
    while let Some(mut i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::StdTableOpen | TokenKind::ArrayTableOpen => {}
            TokenKind::ArrayOpen | TokenKind::InlineTableOpen => {
                depth += 1;
            }
            TokenKind::StdTableClose | TokenKind::ArrayTableClose => {}
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
            TokenKind::Newline if i == 0 => {
                tokens.tokens.remove(0);
                indices.set_next_index(0);
            }
            TokenKind::Newline => {
                let blank_i = i + 1;
                if blank_i < tokens.len() {
                    let actual_newline_count = tokens.tokens[blank_i..]
                        .iter()
                        .take_while(|t| t.kind == TokenKind::Newline)
                        .count();
                    let constrained_newline_count = if i + 1 == tokens.tokens.len() || depth != 0 {
                        0
                    } else {
                        actual_newline_count.clamp(min, max)
                    };
                    if let Some(remove_count) =
                        actual_newline_count.checked_sub(constrained_newline_count)
                    {
                        tokens.tokens.splice(blank_i..blank_i + remove_count, []);
                    } else if let Some(add_count) =
                        constrained_newline_count.checked_sub(actual_newline_count)
                    {
                        tokens
                            .tokens
                            .splice(blank_i..blank_i, (0..add_count).map(|_| TomlToken::NL));
                    }
                    i = blank_i + constrained_newline_count - 1;
                    indices.set_next_index(i + 1);
                }
            }
            TokenKind::Error => {}
        }
    }
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, min: usize, max: usize, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::constrain_blank_lines(&mut tokens, min, max);
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
    fn empty_no_blank() {
        valid("", 0, 0, str![]);
    }

    #[test]
    fn empty_many_blanks() {
        valid("", 3, 10, str![]);
    }

    #[test]
    fn single_key_value_no_blank() {
        valid("a = 5", 0, 0, str!["a = 5"]);
    }

    #[test]
    fn single_key_value_many_blanks() {
        valid("a = 5", 3, 10, str!["a = 5"]);
    }

    #[test]
    fn remove_blank_lines() {
        valid(
            "

a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,

]


g = { a = 1, b = 2 }


",
            0,
            0,
            str![[r#"
a = 5
b = 6
# comment 
# comment
c = 7
[d]
e = 10
f = [
  1,
  2,
]
g = { a = 1, b = 2 }

"#]],
        );
    }

    #[test]
    fn compact_blank_lines() {
        valid(
            "

a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,


]


g = { a = 1, b = 2 }


",
            1,
            1,
            str![[r#"
a = 5

b = 6

# comment 

# comment

c = 7

[d]

e = 10

f = [
  1,
  2,
]

g = { a = 1, b = 2 }


"#]],
        );
    }

    #[test]
    fn add_blank_lines() {
        valid(
            "a = 5
b = 6
# comment 
# comment
c = 7
[d]
e = 10
f = [
  1,
  2,
]
g = { a = 1, b = 2 }",
            2,
            2,
            str![[r#"
a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [
  1,
  2,
]


g = { a = 1, b = 2 }
"#]],
        );
    }

    #[test]
    fn expand_blank_lines() {
        valid(
            "
a = 5

b = 6

# comment 

# comment

c = 7

[d]

e = 10

f = [

  1,

  2,

]

g = { a = 1, b = 2 }
",
            3,
            3,
            str![[r#"
a = 5



b = 6



# comment 



# comment



c = 7



[d]



e = 10



f = [
  1,
  2,
]



g = { a = 1, b = 2 }

"#]],
        );
    }

    #[test]
    fn blank_line_between_array_close_and_table_open() {
        valid(
            r#"
key = [
]

[b]
"#,
            0,
            1,
            str![[r#"
key = [
]

[b]

"#]],
        );
    }
}
