use crate::toml::TomlTokens;

/// Reflow arrays that exceed `max_width` to vertical layout.
#[tracing::instrument]
pub fn reflow_arrays(_tokens: &mut TomlTokens<'_>, _max_width: usize, _tab_spaces: usize) {
    // TODO: implement array reflow
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    use crate::toml::TomlTokens;

    const DEFAULT_TAB_SPACES: usize = 4;

    #[track_caller]
    fn valid(input: &str, max_width: usize, expected: impl IntoData) {
        let mut tokens = TomlTokens::parse(input);
        super::reflow_arrays(&mut tokens, max_width, DEFAULT_TAB_SPACES);
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
    fn short_array_not_reflowed() {
        // Short arrays stay horizontal
        valid(
            r#"deps = ["a", "b"]
"#,
            80,
            str![[r#"
deps = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn long_array_reflowed() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = ["foo", "bar", "baz"]
"#,
            20,
            str![[r#"
deps = ["foo", "bar", "baz"]

"#]],
        );
    }

    #[test]
    fn already_vertical_not_modified() {
        // Already vertical arrays stay unchanged
        valid(
            r#"deps = [
    "foo",
    "bar",
]
"#,
            20,
            str![[r#"
deps = [
    "foo",
    "bar",
]

"#]],
        );
    }

    #[test]
    fn nested_array_reflowed() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"matrix = [[1, 2, 3], [4, 5, 6]]
"#,
            20,
            str![[r#"
matrix = [[1, 2, 3], [4, 5, 6]]

"#]],
        );
    }

    #[test]
    fn deeply_nested_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"x = [[[1]]]
"#,
            5,
            str![[r#"
x = [[[1]]]

"#]],
        );
    }

    #[test]
    fn deeply_nested_partial_reflow() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"x = [[[1]]]
"#,
            10,
            str![[r#"
x = [[[1]]]

"#]],
        );
    }

    #[test]
    fn array_with_inline_table() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = [{name = "foo"}, {name = "bar"}]
"#,
            30,
            str![[r#"
deps = [{name = "foo"}, {name = "bar"}]

"#]],
        );
    }

    #[test]
    fn empty_array_not_reflowed() {
        // Empty arrays stay unchanged
        valid(
            r#"deps = []
"#,
            10,
            str![[r#"
deps = []

"#]],
        );
    }

    #[test]
    fn array_at_exact_max_width() {
        // At exact max_width, no reflow needed
        valid(
            r#"a = [1, 2]
"#,
            10,
            str![[r#"
a = [1, 2]

"#]],
        );
    }

    #[test]
    fn array_one_over_max_width() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"a = [1, 2]
"#,
            9,
            str![[r#"
a = [1, 2]

"#]],
        );
    }

    #[test]
    fn max_width_zero_reflows_everything() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"a = [1]
"#,
            0,
            str![[r#"
a = [1]

"#]],
        );
    }

    #[test]
    fn max_width_max_reflows_nothing() {
        // Very large max_width means no reflow
        valid(
            r#"deps = ["foo", "bar", "baz", "qux", "quux"]
"#,
            usize::MAX,
            str![[r#"
deps = ["foo", "bar", "baz", "qux", "quux"]

"#]],
        );
    }

    #[test]
    fn long_inline_table_not_reflowed() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = [{name = "very-long-name", version = "1.0.0", features = ["a", "b"]}]
"#,
            40,
            str![[r#"
deps = [{name = "very-long-name", version = "1.0.0", features = ["a", "b"]}]

"#]],
        );
    }

    #[test]
    fn inline_table_containing_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"dep = [{features = ["a", "b", "c"]}]
"#,
            20,
            str![[r#"
dep = [{features = ["a", "b", "c"]}]

"#]],
        );
    }

    #[test]
    fn nested_inline_tables() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"items = [{outer = {inner = "value"}}]
"#,
            20,
            str![[r#"
items = [{outer = {inner = "value"}}]

"#]],
        );
    }

    #[test]
    fn array_with_comments() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = ["foo", "bar"] # comment
"#,
            20,
            str![[r#"
deps = ["foo", "bar"] # comment

"#]],
        );
    }

    #[test]
    fn array_with_trailing_comma() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = ["foo", "bar",]
"#,
            15,
            str![[r#"
deps = ["foo", "bar",]

"#]],
        );
    }

    #[test]
    fn very_long_single_element() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"deps = ["this-is-a-very-long-package-name"]
"#,
            20,
            str![[r#"
deps = ["this-is-a-very-long-package-name"]

"#]],
        );
    }

    #[test]
    fn array_in_table_section() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"[package]
keywords = ["cli", "toml", "formatter"]
"#,
            30,
            str![[r#"
[package]
keywords = ["cli", "toml", "formatter"]

"#]],
        );
    }

    #[test]
    fn unicode_values_in_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"names = ["日本語", "中文", "한국어"]
"#,
            20,
            str![[r#"
names = ["日本語", "中文", "한국어"]

"#]],
        );
    }

    #[test]
    fn multiline_string_in_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"items = ["""
multi
line
"""]
"#,
            10,
            str![[r#"
items = ["""
multi
line
"""]

"#]],
        );
    }

    #[test]
    fn unclosed_array_not_panics() {
        // Malformed input: should gracefully skip rather than panic
        // Note: not using valid() since output is invalid TOML
        let input = r#"deps = ["foo", "bar"
"#;
        let mut tokens = TomlTokens::parse(input);
        super::reflow_arrays(&mut tokens, 10, DEFAULT_TAB_SPACES);
        let actual = tokens.to_string();
        assert_data_eq!(
            &actual,
            str![[r#"
deps = ["foo", "bar"

"#]]
        );
    }

    #[test]
    fn dotted_key_width_included() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            23,
            str![[r#"
foo.bar.baz = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn dotted_key_at_exact_width() {
        valid(
            r#"foo.bar.baz = ["a", "b"]
"#,
            24,
            str![[r#"
foo.bar.baz = ["a", "b"]

"#]],
        );
    }

    #[test]
    fn quoted_key() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#""my.key" = ["x", "y"]
"#,
            15,
            str![[r#"
"my.key" = ["x", "y"]

"#]],
        );
    }

    #[test]
    fn literal_strings() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"paths = ['foo', 'bar']
"#,
            15,
            str![[r#"
paths = ['foo', 'bar']

"#]],
        );
    }

    #[test]
    fn mixed_types_in_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"mixed = [1, "two", true, 3.14]
"#,
            20,
            str![[r#"
mixed = [1, "two", true, 3.14]

"#]],
        );
    }

    #[test]
    fn multiple_arrays_same_section() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"[pkg]
a = [1, 2, 3]
b = [4, 5, 6, 7, 8]
"#,
            15,
            str![[r#"
[pkg]
a = [1, 2, 3]
b = [4, 5, 6, 7, 8]

"#]],
        );
    }

    #[test]
    fn array_at_start_of_file() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"x = ["a", "b", "c"]
"#,
            15,
            str![[r#"
x = ["a", "b", "c"]

"#]],
        );
    }

    #[test]
    fn empty_string_elements() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"x = ["", "a", ""]
"#,
            12,
            str![[r#"
x = ["", "a", ""]

"#]],
        );
    }

    #[test]
    fn integer_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"nums = [1, 2, 3, 4, 5]
"#,
            15,
            str![[r#"
nums = [1, 2, 3, 4, 5]

"#]],
        );
    }

    #[test]
    fn boolean_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"flags = [true, false, true]
"#,
            20,
            str![[r#"
flags = [true, false, true]

"#]],
        );
    }

    #[test]
    fn float_array() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"values = [1.5, 2.25, 3.125]
"#,
            20,
            str![[r#"
values = [1.5, 2.25, 3.125]

"#]],
        );
    }

    #[test]
    fn nested_only_inner_exceeds() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"x = [[1, 2, 3, 4]]
"#,
            12,
            str![[r#"
x = [[1, 2, 3, 4]]

"#]],
        );
    }

    #[test]
    fn very_long_key_array_still_reflows() {
        // Currently: no reflow (stub does nothing)
        valid(
            r#"this_is_a_very_long_key = [1]
"#,
            20,
            str![[r#"
this_is_a_very_long_key = [1]

"#]],
        );
    }
}
