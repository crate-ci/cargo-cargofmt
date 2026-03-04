use crate::toml::{TokenKind, TomlToken, TomlTokens};

pub fn sort_table_hierarchy(tokens: &mut TomlTokens<'_>) {
    let mut table_chunks = collect_table_chunks(tokens);

    if table_chunks.len() <= 1 {
        return;
    }

    // We prioritize [workspace] then [package] to align with the roadmap in #28.
    table_chunks.sort_by(|a, b| {
        let get_priority = |path: &[String]| -> i32 {
            match path.first().map(|s| s.as_str()) {
                Some("workspace") => 1,
                Some("package") => 2,
                _ => 3, // All other tables (dependencies, features, etc.)
            }
        };

        let a_prio = get_priority(&a.header_path);
        let b_prio = get_priority(&b.header_path);

        if a_prio != b_prio {
            a_prio.cmp(&b_prio)
        } else {
            // Lexicographical sort (e.g. [package] before [package.metadata])
            a.header_path.cmp(&b.header_path)
        }
    });

    tokens.tokens.clear();
    for (i, chunk) in table_chunks.into_iter().enumerate() {
        if i > 0 {
            tokens.tokens.push(TomlToken::NL);
        }
        tokens.tokens.extend(chunk.all_tokens);
    }

    // To ensure the file ends with a newline character.
    if let Some(last) = tokens.tokens.last() {
        if last.kind != TokenKind::Newline {
            tokens.tokens.push(TomlToken::NL);
        }
    }
}

#[derive(Debug)]
struct TableChunk<'a> {
    header_path: Vec<String>,
    all_tokens: Vec<TomlToken<'a>>,
}

// Because the parser yields brackets and keys as discrete tokens we peek 
// forward to identify the table path for sorting.
fn collect_table_chunks<'a>(tokens: &TomlTokens<'a>) -> Vec<TableChunk<'a>> {
    let mut chunks = Vec::new();
    let mut current_tokens: Vec<TomlToken<'a>> = Vec::new();
    let mut current_header_path: Vec<String> = Vec::new();

    let mut i = 0;
    while i < tokens.tokens.len() {
        let token = &tokens.tokens[i];

        if token.kind == TokenKind::StdTableOpen || token.kind == TokenKind::ArrayTableOpen {
            
            // Pull comments/whitespace into this chunk
            let mut leading_stuff: Vec<TomlToken<'a>> = Vec::new();
            while let Some(last_t) = current_tokens.last() {
                if matches!(last_t.kind, TokenKind::Comment | TokenKind::Whitespace | TokenKind::Newline) {
                    leading_stuff.insert(0, current_tokens.pop().unwrap());
                } else {
                    break;
                }
            }

            if !current_tokens.is_empty() {
                chunks.push(TableChunk {
                    header_path: current_header_path.clone(),
                    all_tokens: current_tokens.split_off(0),
                });
            }

            current_tokens.extend(leading_stuff);

            // Scan for the table name parts (e.g. ["package", "metadata"])
            let mut j = i + 1;
            let mut path_parts = Vec::new();
            while j < tokens.tokens.len() {
                let t_next = &tokens.tokens[j];
                if t_next.kind == TokenKind::StdTableClose {
                    break;
                }
                if t_next.kind == TokenKind::SimpleKey || t_next.kind == TokenKind::Scalar {
                    path_parts.push(t_next.raw.trim().to_string());
                }
                j += 1;
            }
            current_header_path = path_parts;
        }

        current_tokens.push(token.clone());
        i += 1;
    }

    // Finalization
    if !current_tokens.is_empty() {
        chunks.push(TableChunk {
            header_path: current_header_path,
            all_tokens: current_tokens,
        });
    }

    chunks
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_hierarchical_sort() {
        let input = r#"
[dependencies]
anyhow = "1.0"
[package.metadata]
custom = true
# Sticky comment
[package]
name = "test"
[workspace]
members = ["."]
"#;
        let mut tokens = TomlTokens::parse(input);
        sort_table_hierarchy(&mut tokens);
        let output = tokens.to_string();
        
        assert!(output.find("[workspace]").unwrap() < output.find("[package]").unwrap());
        assert!(output.find("[package]").unwrap() < output.find("[package.metadata]").unwrap());
        assert!(output.find("[package.metadata]").unwrap() < output.find("[dependencies]").unwrap());
        
        assert!(output.find("# Sticky comment").unwrap() < output.find("[package]").unwrap());
    }
}
