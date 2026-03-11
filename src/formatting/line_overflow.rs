use crate::toml::TomlTokens;

pub fn check_line_overflow(
    _tokens: &TomlTokens<'_>,
    _max_width: usize,
    _tab_spaces: usize,
) -> Vec<(usize, usize)> {
    vec![] //todo
}

#[cfg(test)]
mod test {
    use crate::toml::TomlTokens;

    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, max_width: usize, tab_spaces: usize, expected: impl IntoData) {
        let tokens = TomlTokens::parse(input);
        let actual = super::check_line_overflow(&tokens, max_width, tab_spaces);
        let actual_str = format!("{:?}", actual);
        assert_data_eq!(&actual_str, expected);
    }

    #[test]
    fn no_overflow() {
        let input = "[package]\nname = \"app\"\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_table_header() {
        let input = "[very-long-table-header-name]\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn exempt_standalone_comment() {
        let input = "# A very long comment line\n";
        valid(input, 8, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn exempt_inline_comment() {
        let input = "key = \"value\"  # very long comment here\n";
        valid(input, 10, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_inline_comment() {
        let input = "very-long-key-name = 1 # short comment\n";
        valid(input, 15, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_from_long_key() {
        let input = "a_very_long_key_cause_overflow = \"b\"\n";
        valid(input, 10, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn exempt_string_literal() {
        let input = "description = \"https://docs.rs/test-overflow-with-super-long-paths-and-extra-sections-that-keep-going-forever\"\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn long_key_with_string_literal_errors() {
        let input = "very_long_key_name_here = \"short\"\n";
        valid(input, 20, 4, str![[r#"[]"#]]);
    }
}
