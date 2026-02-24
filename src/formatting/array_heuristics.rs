use crate::toml::{TokenKind, TomlToken};

pub fn apply_array_heuristics(tokens: &mut crate::toml::TomlTokens<'_>, threshold: usize) {
    let mut i = 0;
    while i < tokens.tokens.len() {
        if tokens.tokens[i].kind == TokenKind::ArrayOpen {
            let start_index = i;
            let mut has_long_element = false;
            let mut end_index = None;

            for j in (i + 1)..tokens.tokens.len() {
                match tokens.tokens[j].kind {
                    TokenKind::Scalar => {
                        let raw_text = tokens.tokens[j].raw.as_ref();
                        let clean_text = raw_text.trim_matches(|c| c == '"' || c == '\'');
                        if clean_text.chars().count() > threshold {
                            has_long_element = true;
                        }
                    }
                    TokenKind::ArrayClose => {
                        end_index = Some(j);
                        break;
                    }
                    _ => {}
                }
            }

            if let (true, Some(mut current_end)) = (has_long_element, end_index) {
                let mut j = start_index + 1;
                while j <= current_end {
                    let prev_kind = tokens.tokens[j - 1].kind;
                    
                    if (prev_kind == TokenKind::ArrayOpen || prev_kind == TokenKind::ValueSep)
                        && tokens.tokens[j].kind != TokenKind::Newline
                    {
                        tokens.tokens.insert(j, TomlToken::NL);
                        current_end += 1;
                        j += 1;

                        if j < tokens.tokens.len() && tokens.tokens[j].kind == TokenKind::Whitespace {
                            tokens.tokens.remove(j);
                            current_end -= 1;
                        }
                    }
                    j += 1;
                }
                i = current_end; 
            } else if let Some(e) = end_index {
                i = e;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;

    #[track_caller]
    fn check(input: &str, threshold: usize, expected: impl snapbox::IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::apply_array_heuristics(&mut tokens, threshold);
        let actual = tokens.to_string();
        assert_data_eq!(actual, expected);
    }

    #[test]
    fn array_stays_single_line_when_under_threshold() {
        check(
            r#"features = ["abc", "def"]"#,
            10,
            str![r#"features = ["abc", "def"]"#],
        );
    }

    #[test]
    fn array_wraps_completely_when_exceeding_threshold() {
        check(
            r#"features = ["long_element", "a", "b"]"#,
            5,
            str![[r#"features = [
"long_element",
"a",
"b"]"#]], 
        );
    }
}
