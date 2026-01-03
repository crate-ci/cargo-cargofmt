use crate::toml::TomlTokens;

/// Normalize array layouts based on `array_width`.
///
/// - Expands horizontal arrays to vertical when they exceed `array_width`
/// - Collapses vertical arrays to horizontal when they fit within `array_width`
/// - Normalizes mixed-style arrays to the appropriate format
/// - Preserves arrays containing comments (no collapse)
#[tracing::instrument]
pub fn reflow_arrays(_tokens: &mut TomlTokens<'_>, _array_width: usize, _tab_spaces: usize) {}

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
    fn unclosed_array_not_panics() {
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
        valid(
            r#"this_is_a_very_long_key = [1]
"#,
            20,
            str![[r#"
this_is_a_very_long_key = [1]

"#]],
        );
    }

    #[test]
    fn vertical_collapses_when_fits() {
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
    fn mixed_style_with_comment_preserved() {
        // Mixed-style arrays with comments are preserved as-is
        // (not normalized) to avoid semantic changes from comment displacement
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
    fn grouped_comments_preserved_on_overflow() {
        // When any grouped line exceeds max_width, rustfmt splits ALL groups.
        // We preserve grouping to avoid losing semantic structure.
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
    fn standalone_comments_preserved_on_collapse() {
        // Standalone comments between elements would be moved to trailing
        // position on previous element during collapse, changing semantics.
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
    fn nested_vertical_collapses() {
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
        // `a = ["日"]` = 10 display columns
        // At max_width=10: should NOT reflow
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
        // `a = ["🎉"]` = 10 display columns
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
        // "é" as e + combining acute (U+0301) is 2 codepoints but 1 display column
        // `a = ["é"]` with combining = 10 codepoints but 9 display columns
        // At max_width=9: should NOT reflow (display width fits)
        valid(
            "a = [\"e\u{0301}\"]\n",
            9,
            // Expected output preserves decomposed form (e + combining acute)
            "a = [\"e\u{0301}\"]\n",
        );
    }

    #[test]
    fn combining_character_reflows_at_boundary() {
        // At max_width=8: should reflow (display width 9 > 8)
        valid(
            "a = [\"e\u{0301}\"]\n",
            8,
            // Expected output preserves decomposed form (e + combining acute)
            "a = [\"e\u{0301}\"]\n",
        );
    }

    #[test]
    fn vertical_cjk_collapses_at_correct_width() {
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
        let nested = "x = [[[[[[[[[[1]]]]]]]]]]\n";
        valid(
            nested,
            5,
            str![[r#"
x = [[[[[[[[[[1]]]]]]]]]]

"#]],
        );
    }

    // Tab handling tests

    #[test]
    fn tabs_in_array_counted_as_tab_spaces() {
        valid(
            "x = [\t1]\n",
            11,
            str![[r#"
x = [	1]

"#]],
        );
    }

    #[test]
    fn tabs_in_array_cause_reflow_at_boundary() {
        valid(
            "x = [\t1]\n",
            10,
            str![[r#"
x = [	1]

"#]],
        );
    }

    #[test]
    fn tabs_between_elements_normalized_on_collapse() {
        valid(
            "x = [\n\t1,\n\t2,\n]\n",
            40,
            str![[r#"
x = [
	1,
	2,
]

"#]],
        );
    }

    #[test]
    fn multiple_tabs_expand_correctly() {
        valid(
            "x = [\t\t1]\n",
            12,
            str![[r#"
x = [		1]

"#]],
        );
    }

    // Deeply nested mixed collapse/expand tests

    #[test]
    fn vertical_outer_with_long_horizontal_inner_expands_inner() {
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
        // String with various special chars that don't need escaping
        // `x = ["a-b_c.d"]` = 15 columns
        // At max_width=14: should reflow (but not yet implemented)
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
