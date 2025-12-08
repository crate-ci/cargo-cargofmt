#[tracing::instrument]
pub fn normalize_space_separators(_tokens: &mut crate::toml::TomlTokens<'_>) {}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::normalize_space_separators(&mut tokens);
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
    fn empty() {
        valid("", str![]);
    }

    #[test]
    fn key_value_without_spaces() {
        valid("key=5", str!["key=5"]);
    }

    #[test]
    fn key_value_with_extra_spaces() {
        valid("key   =    5", str!["key   =    5"]);
    }

    #[test]
    fn key_value_with_tab() {
        valid("key\t=\t5", str!["key	=	5"]);
    }

    #[test]
    fn comment_without_spaces() {
        valid("key = 5#Hello", str!["key = 5#Hello"]);
    }

    #[test]
    fn comment_with_extra_spaces() {
        valid("key = 5    #    Hello", str!["key = 5    #    Hello"]);
    }

    #[test]
    fn comment_with_tab() {
        valid("key = 5\t#\tHello", str!["key = 5	#	Hello"]);
    }

    #[test]
    fn array_empty() {
        valid("key = []", str!["key = []"]);
    }

    #[test]
    fn array_spaces() {
        valid("key = [    ]", str!["key = [    ]"]);
    }

    #[test]
    fn array_tab() {
        valid("key = [\t]", str!["key = [	]"]);
    }

    #[test]
    fn array_value_without_spaces() {
        valid("key = [5]", str!["key = [5]"]);
    }

    #[test]
    fn array_value_with_extra_spaces() {
        valid("key = [    5    ]", str!["key = [    5    ]"]);
    }

    #[test]
    fn array_value_with_tab() {
        valid("key = [\t5\t]", str!["key = [	5	]"]);
    }

    #[test]
    fn value_sep_without_spaces() {
        valid("key = [5,6]", str!["key = [5,6]"]);
    }

    #[test]
    fn value_sep_with_extra_spaces() {
        valid("key = [5  ,  6]", str!["key = [5  ,  6]"]);
    }

    #[test]
    fn value_sep_with_tab() {
        valid("key = [5\t,\t6]", str!["key = [5	,	6]"]);
    }

    #[test]
    fn value_sep_with_newline() {
        valid(
            "key = [5
,
6]",
            str![[r#"
key = [5
,
6]
"#]],
        );
    }

    #[test]
    fn value_sep_with_comment() {
        valid(
            "key = [5 # hello
, # goodbye
6]",
            str![[r#"
key = [5 # hello
, # goodbye
6]
"#]],
        );
    }

    #[test]
    fn value_sep_trailing_without_spaces() {
        valid("key = [5,]", str!["key = [5,]"]);
    }

    #[test]
    fn value_sep_trailing_with_extra_spaces() {
        valid("key = [5  ,  ]", str!["key = [5  ,  ]"]);
    }

    #[test]
    fn value_sep_trailing_with_tab() {
        valid("key = [5\t,\t]", str!["key = [5	,	]"]);
    }

    #[test]
    fn inline_table_empty() {
        valid("key = {}", str!["key = {}"]);
    }

    #[test]
    fn inline_table_spaces() {
        valid("key = {    }", str!["key = {    }"]);
    }

    #[test]
    fn inline_table_tab() {
        valid("key = {\t}", str!["key = {	}"]);
    }

    #[test]
    fn inline_table_value_without_spaces() {
        valid("key = {key=5}", str!["key = {key=5}"]);
    }

    #[test]
    fn inline_table_value_with_extra_spaces() {
        valid(
            "key = {    key     =    5    }",
            str!["key = {    key     =    5    }"],
        );
    }

    #[test]
    fn inline_table_value_with_tab() {
        valid("key = {\tkey\t=\t5\t}", str!["key = {	key	=	5	}"]);
    }

    #[test]
    fn inline_table_sep_without_spaces() {
        valid("key = {a=5,b=6}", str!["key = {a=5,b=6}"]);
    }

    #[test]
    fn inline_table_sep_with_extra_spaces() {
        valid("key = {a=5    ,    b=6}", str!["key = {a=5    ,    b=6}"]);
    }

    #[test]
    fn inline_table_sep_with_tab() {
        valid("key = {a=5\t,\tb=6}", str!["key = {a=5	,	b=6}"]);
    }

    #[test]
    fn table_without_spaces() {
        valid("[key]", str!["[key]"]);
    }

    #[test]
    fn table_with_extra_spaces() {
        valid("[   key    ]", str!["[   key    ]"]);
    }

    #[test]
    fn table_with_tab() {
        valid("[\tkey\t]", str!["[	key	]"]);
    }

    #[test]
    fn array_of_tables_without_spaces() {
        valid("[[key]]", str!["[[key]]"]);
    }

    #[test]
    fn array_of_tables_with_extra_spaces() {
        valid("[[   key    ]]", str!["[[   key    ]]"]);
    }

    #[test]
    fn array_of_tables_with_tab() {
        valid("[[\tkey\t]]", str!["[[	key	]]"]);
    }

    #[test]
    fn key_sep_without_spaces() {
        valid("a.b = 5", str!["a.b = 5"]);
    }

    #[test]
    fn key_sep_with_extra_spaces() {
        valid("a    .    b = 5", str!["a    .    b = 5"]);
    }

    #[test]
    fn key_sep_with_tab() {
        valid("a\t.\tb = 5", str!["a	.	b = 5"]);
    }
}
