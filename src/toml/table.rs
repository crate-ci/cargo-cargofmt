use super::TokenIndices;
use super::TokenKind;
use super::TomlTokens;

pub struct Table {
    pub name: Vec<String>,
    /// First token of the table's range, including any leading comments.
    pub start: usize,
    /// Equal to next table's header index or token count.
    pub end: usize,
    pub is_array_table: bool,
}

impl Table {
    pub fn new(tokens: &TomlTokens<'_>) -> Vec<Table> {
        // First pass: find all headers and their starts (including leading comments)
        let mut header_info: Vec<(usize, usize, bool)> = Vec::new(); // (header_idx, start, is_array)
        let mut indices = TokenIndices::new();

        while let Some(i) = indices.next_index(tokens) {
            let kind = tokens.tokens[i].kind;
            if matches!(kind, TokenKind::StdTableOpen | TokenKind::ArrayTableOpen) {
                let start = find_start(tokens, i);
                header_info.push((i, start, kind == TokenKind::ArrayTableOpen));
            }
        }

        // Second pass: construct tables with end boundaries
        let mut tables = Vec::new();
        for (idx, &(header_idx, start, is_array_table)) in header_info.iter().enumerate() {
            let end = match header_info.get(idx + 1) {
                Some(&(next_header_idx, _, _)) => next_header_idx,
                None => tokens.len(),
            };
            let (name, _) = parse_table_name(tokens, header_idx + 1);
            tables.push(Table {
                name,
                start,
                end,
                is_array_table,
            });
        }

        tables
    }
}

fn find_start(tokens: &TomlTokens<'_>, header_idx: usize) -> usize {
    if header_idx == 0 {
        return 0;
    }

    let mut newline_count = 0;
    let mut indices = TokenIndices::from_index(header_idx);

    while let Some(i) = indices.prev_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::Comment => {
                // Adjacent comment is a leading comment
                if newline_count == 1 {
                    return i;
                }
                return header_idx;
            }
            TokenKind::Newline => {
                newline_count += 1;
                if newline_count > 1 {
                    return header_idx;
                }
            }
            TokenKind::Whitespace => {}
            _ => return header_idx,
        }
    }

    header_idx
}

fn parse_table_name(tokens: &TomlTokens<'_>, start: usize) -> (Vec<String>, usize) {
    let mut name = Vec::new();
    let mut indices = TokenIndices::from_index(start);

    while let Some(i) = indices.next_index(tokens) {
        match tokens.tokens[i].kind {
            TokenKind::SimpleKey => {
                let token = &tokens.tokens[i];
                name.push(token.decoded.as_ref().unwrap_or(&token.raw).to_string());
            }
            TokenKind::KeySep | TokenKind::Whitespace => {}
            _ => {
                return (name, i);
            }
        }
    }

    (name, tokens.len().saturating_sub(1))
}
