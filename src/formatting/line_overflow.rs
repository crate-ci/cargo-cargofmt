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
        let input = r#"[package]
name = "app"
"#;
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_table_header() {
        let input = r#"[very-long-table-header-name]"#;
        valid(input, 20, 4, str![[r#"[]"#]]); // expected - [(1, 29)]
    }

    #[test]
    fn exempt_standalone_comment() {
        let input = r#"# A very long comment line
"#;
        valid(input, 8, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn exempt_inline_comment() {
        let input = r#"key = "value"  # very long comment here
"#;
        valid(input, 10, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn overflow_inline_comment() {
        let input = r#"very-long-key-name = 1 # short comment
"#;
        valid(input, 15, 4, str![[r#"[]"#]]); // expected - [(1, 38)]
    }

    #[test]
    fn overflow_from_long_key() {
        let input = r#"a_very_long_key_cause_overflow = "b"
"#;
        valid(input, 10, 4, str![[r#"[]"#]]); // expected - [(1, 36)]
    }

    #[test]
    fn exempt_string_literal() {
        let input = r#"description = "https://docs.rs/test-overflow-with-super-long-paths-and-extra-sections-that-keep-going-forever"
"#;
        valid(input, 20, 4, str![[r#"[]"#]]);
    }

    #[test]
    fn long_key_with_string_literal_errors() {
        let input = r#"very_long_key_name_here = "short"
"#;
        valid(input, 20, 4, str![[r#"[]"#]]); // expected - [(1, 33)]
    }
}
