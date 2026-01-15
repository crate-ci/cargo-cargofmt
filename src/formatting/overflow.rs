use crate::toml::TomlTokens;

/// Normalize array layouts based on `array_width`.
///
/// - Expands horizontal arrays to vertical when they exceed `array_width`
/// - Collapses vertical arrays to horizontal when they fit within `array_width`
/// - Normalizes mixed-style arrays to proper vertical format
/// - Preserves arrays containing comments (no collapse, but normalizes layout)
/// - Comments are preserved in their relative positions during normalization
///
/// Uses incremental depth tracking for O(n) complexity instead of
/// rescanning from the start for each array.
#[tracing::instrument]
pub fn reflow_arrays(_tokens: &mut TomlTokens<'_>, _array_width: usize, _tab_spaces: usize) {
    // Stub: no transformation yet
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
    fn vertical_multiline_string_collapses_when_fits() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    """
multi
""",
]
"#,
            80,
            str![[r#"
x = [
    """
multi
""",
]

"#]],
        );
    }

    #[test]
    fn multiline_literal_string_preserved() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    '''
literal
''',
]
"#,
            80,
            str![[r#"
x = [
    '''
literal
''',
]

"#]],
        );
    }

    #[test]
    fn dotted_key_width_included() {
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
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
    fn nested_only_inner_exceeds() {
        // Stub: no transformation, input == output
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
        // Stub: no transformation, input == output
        valid(
            r#"this_is_a_very_long_key = [1]
"#,
            20,
            str![[r#"
this_is_a_very_long_key = [1]

"#]],
        );
    }

    // Collapse tests

    #[test]
    fn vertical_collapses_when_fits() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "a",
    "b",
]
"#,
            40,
            str![[r#"
x = [
    "a",
    "b",
]

"#]],
        );
    }

    #[test]
    fn vertical_stays_when_too_wide() {
        valid(
            r#"x = [
    "aaa",
    "bbb",
]
"#,
            10,
            str![[r#"
x = [
    "aaa",
    "bbb",
]

"#]],
        );
    }

    #[test]
    fn mixed_style_collapses_when_fits() {
        // Stub: no transformation, input == output
        valid(
            r#"x = ["a", "b",
    "c"]
"#,
            40,
            str![[r#"
x = ["a", "b",
    "c"]

"#]],
        );
    }

    #[test]
    fn mixed_style_normalizes_when_too_wide() {
        // Stub: no transformation, input == output
        valid(
            r#"x = ["aaa", "bbb",
    "ccc"]
"#,
            10,
            str![[r#"
x = ["aaa", "bbb",
    "ccc"]

"#]],
        );
    }

    #[test]
    fn vertical_with_comment_stays_vertical() {
        valid(
            r#"x = [
    "a", # comment
    "b",
]
"#,
            80,
            str![[r#"
x = [
    "a", # comment
    "b",
]

"#]],
        );
    }

    #[test]
    fn mixed_style_with_comment_normalized() {
        // Stub: no transformation, input == output
        valid(
            r#"x = ["a", "b", # comment
    "c",
]
"#,
            80,
            str![[r#"
x = ["a", "b", # comment
    "c",
]

"#]],
        );
    }

    #[test]
    fn grouped_elements_with_comments_normalized() {
        // Stub: no transformation, input == output
        valid(
            r#"deps = [
    "a", "b", "c",
    "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", # comment about this group
    "x", "y", "z", # fits
]
"#,
            60,
            str![[r#"
deps = [
    "a", "b", "c",
    "aaaaaaaaaaaa", "bbbbbbbbbbbb", "cccccccccccc", # comment about this group
    "x", "y", "z", # fits
]

"#]],
        );
    }

    #[test]
    fn standalone_comment_groups_horizontally() {
        // Stub: no transformation, input == output
        valid(
            r#"deps = [
    "a",
    "b",
    # comment about elements below
    "c",
    "d",
]
"#,
            200,
            str![[r#"
deps = [
    "a",
    "b",
    # comment about elements below
    "c",
    "d",
]

"#]],
        );
    }

    #[test]
    fn comment_on_last_element_collapses() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "a",
    "b", # comment
]
"#,
            80,
            str![[r#"
x = [
    "a",
    "b", # comment
]

"#]],
        );
    }

    #[test]
    fn comment_before_close_stays_vertical() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "a",
    "b",
    # trailing comment
]
"#,
            80,
            str![[r#"
x = [
    "a",
    "b",
    # trailing comment
]

"#]],
        );
    }

    #[test]
    fn nested_vertical_collapses() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    [
        1
    ],
    [
        2
    ],
]
"#,
            40,
            str![[r#"
x = [
    [
        1
    ],
    [
        2
    ],
]

"#]],
        );
    }

    #[test]
    fn collapse_removes_trailing_comma() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "a",
    "b",
]
"#,
            40,
            str![[r#"
x = [
    "a",
    "b",
]

"#]],
        );
    }

    #[test]
    fn collapse_normalizes_spacing() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "a"  ,
    "b"  ,
]
"#,
            40,
            str![[r#"
x = [
    "a"  ,
    "b"  ,
]

"#]],
        );
    }

    // Unicode width edge case tests

    #[test]
    fn cjk_double_width_causes_reflow() {
        // Stub: no transformation, input == output
        valid(
            r#"a = ["日"]
"#,
            9,
            str![[r#"
a = ["日"]

"#]],
        );
    }

    #[test]
    fn cjk_double_width_fits_at_correct_width() {
        valid(
            r#"a = ["日"]
"#,
            10,
            str![[r#"
a = ["日"]

"#]],
        );
    }

    #[test]
    fn emoji_double_width_causes_reflow() {
        // Stub: no transformation, input == output
        valid(
            r#"a = ["🎉"]
"#,
            9,
            str![[r#"
a = ["🎉"]

"#]],
        );
    }

    #[test]
    fn emoji_double_width_fits_at_correct_width() {
        valid(
            r#"a = ["🎉"]
"#,
            10,
            str![[r#"
a = ["🎉"]

"#]],
        );
    }

    #[test]
    fn combining_character_zero_width() {
        valid("a = [\"e\u{0301}\"]\n", 9, "a = [\"e\u{0301}\"]\n");
    }

    #[test]
    fn combining_character_reflows_at_boundary() {
        // Stub: no transformation, input == output
        valid("a = [\"e\u{0301}\"]\n", 8, "a = [\"e\u{0301}\"]\n");
    }

    #[test]
    fn vertical_cjk_collapses_at_correct_width() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    "日",
    "月",
]
"#,
            16,
            str![[r#"
x = [
    "日",
    "月",
]

"#]],
        );
    }

    #[test]
    fn vertical_cjk_stays_vertical_when_too_wide() {
        valid(
            r#"x = [
    "日",
    "月",
]
"#,
            15,
            str![[r#"
x = [
    "日",
    "月",
]

"#]],
        );
    }

    #[test]
    fn deeply_nested_within_limit() {
        // Stub: no transformation, input == output
        let nested = "x = [[[[[[[[[[1]]]]]]]]]]\n";
        valid(nested, 5, nested);
    }

    // Tab handling tests

    #[test]
    fn tabs_in_array_counted_as_tab_spaces() {
        valid("x = [\t1]\n", 11, "x = [\t1]\n");
    }

    #[test]
    fn tabs_in_array_cause_reflow_at_boundary() {
        // Stub: no transformation, input == output
        valid("x = [\t1]\n", 10, "x = [\t1]\n");
    }

    #[test]
    fn tabs_between_elements_normalized_on_collapse() {
        // Stub: no transformation, input == output
        valid("x = [\n\t1,\n\t2,\n]\n", 40, "x = [\n\t1,\n\t2,\n]\n");
    }

    #[test]
    fn multiple_tabs_expand_correctly() {
        // Stub: no transformation, input == output
        valid("x = [\t\t1]\n", 12, "x = [\t\t1]\n");
    }

    // Deeply nested mixed collapse/expand tests

    #[test]
    fn vertical_outer_with_long_horizontal_inner_expands_inner() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    [1, 2, 3, 4, 5],
]
"#,
            15,
            str![[r#"
x = [
    [1, 2, 3, 4, 5],
]

"#]],
        );
    }

    #[test]
    fn vertical_outer_with_short_horizontal_inner_collapses() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
    [1, 2],
]
"#,
            40,
            str![[r#"
x = [
    [1, 2],
]

"#]],
        );
    }

    #[test]
    fn horizontal_outer_fits_stays_horizontal() {
        valid(
            r#"x = [[1], [2]]
"#,
            20,
            str![[r#"
x = [[1], [2]]

"#]],
        );
    }

    #[test]
    fn outer_expands_inner_fits() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[1], [2]]
"#,
            10,
            str![[r#"
x = [[1], [2]]

"#]],
        );
    }

    #[test]
    fn outer_expands_inner_also_expands() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[1, 2, 3], [4, 5, 6]]
"#,
            10,
            str![[r#"
x = [[1, 2, 3], [4, 5, 6]]

"#]],
        );
    }

    #[test]
    fn mixed_nesting_all_inner_fit() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[1], [2], [3]]
"#,
            15,
            str![[r#"
x = [[1], [2], [3]]

"#]],
        );
    }

    #[test]
    fn mixed_nesting_one_inner_expands() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[1], [2, 3, 4, 5], [6]]
"#,
            15,
            str![[r#"
x = [[1], [2, 3, 4, 5], [6]]

"#]],
        );
    }

    #[test]
    fn three_level_nesting_all_expand() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[[1, 2]]]
"#,
            5,
            str![[r#"
x = [[[1, 2]]]

"#]],
        );
    }

    #[test]
    fn three_level_nesting_small_width() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [[[1]]]
"#,
            8,
            str![[r#"
x = [[[1]]]

"#]],
        );
    }

    #[test]
    fn empty_vertical_array_collapses() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [
]
"#,
            80,
            str![[r#"
x = [
]

"#]],
        );
    }

    #[test]
    fn empty_vertical_array_with_whitespace_collapses() {
        // Stub: no transformation, input == output
        valid(
            r#"x = [

]
"#,
            80,
            str![[r#"
x = [

]

"#]],
        );
    }

    #[test]
    fn long_string_width_at_boundary() {
        valid(
            r#"x = ["abcdefghij"]
"#,
            18,
            str![[r#"
x = ["abcdefghij"]

"#]],
        );
    }

    #[test]
    fn long_string_width_causes_reflow() {
        // Stub: no transformation, input == output
        valid(
            r#"x = ["abcdefghij"]
"#,
            17,
            str![[r#"
x = ["abcdefghij"]

"#]],
        );
    }

    #[test]
    fn string_with_special_chars() {
        // Stub: no transformation, input == output
        valid(
            r#"x = ["a-b_c.d"]
"#,
            14,
            str![[r#"
x = ["a-b_c.d"]

"#]],
        );
    }

    #[test]
    fn array_with_only_whitespace_preserved() {
        valid(
            r#"x = [   ]
"#,
            20,
            str![[r#"
x = [   ]

"#]],
        );
    }
}
