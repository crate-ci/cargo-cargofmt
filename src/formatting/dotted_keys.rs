use crate::toml::TomlTokens;

#[tracing::instrument(skip_all)]
pub fn sort_dotted_key_hierarchy(_tokens: &mut TomlTokens<'_>) {}

#[cfg(test)]
mod test {
    use snapbox::IntoData;
    use snapbox::assert_data_eq;
    use snapbox::str;

    #[track_caller]
    fn valid(input: &str, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::sort_dotted_key_hierarchy(&mut tokens);
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
    fn empty_input() {
        valid("", str![]);
    }

    #[test]
    fn no_dotted_keys() {
        valid(
            r#"name = "foo"
version = "1.0"
"#,
            str![[r#"
name = "foo"
version = "1.0"

"#]],
        );
    }

    #[test]
    fn already_grouped() {
        // already in order, must not be modified
        valid(
            r#"apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"
"#,
            str![[r#"
apple.type = "fruit"
apple.skin = "thin"
orange.type = "citrus"

"#]],
        );
    }

    #[test]
    fn basic_grouping() {
        // core case from issue #56
        valid(
            r#"apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn three_groups_interleaved() {
        valid(
            r#"a.x = 1
b.x = 2
c.x = 3
a.y = 4
b.y = 5
c.y = 6
"#,
            str![[r#"
a.x = 1
b.x = 2
c.x = 3
a.y = 4
b.y = 5
c.y = 6

"#]],
        );
    }

    #[test]
    fn inside_table_section() {
        valid(
            r#"[fruits]
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
[fruits]
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn multiple_sections_sorted_independently() {
        valid(
            r#"[a]
x.foo = 1
y.foo = 2
x.bar = 3
[b]
p.foo = 1
q.foo = 2
p.bar = 3
"#,
            str![[r#"
[a]
x.foo = 1
y.foo = 2
x.bar = 3
[b]
p.foo = 1
q.foo = 2
p.bar = 3

"#]],
        );
    }

    #[test]
    fn plain_key_pushed_after_its_group() {
        // plain key between two apple entries ends up after them
        valid(
            r#"apple.type = "fruit"
version = "1.0"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
version = "1.0"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn comment_travels_with_following_entry() {
        // comment before orange moves with it
        valid(
            r#"apple.type = "fruit"
# the orange entry
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
# the orange entry
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn leading_comment_before_all_keys() {
        // leading section comment stays at the top when entries are reordered
        valid(
            r#"# section header comment
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
# section header comment
apple.type = "fruit"
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn comment_between_keys() {
        // comment between two entries travels with the entry that follows it
        valid(
            r#"apple.type = "fruit"
# orange entry
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
# orange entry
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn newline_and_comment_between_keys() {
        // blank line before comment — both travel with the following entry
        valid(
            r#"apple.type = "fruit"

# orange entry
orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"

# orange entry
orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn comment_and_newline_between_keys() {
        // comment then blank line before the next entry — both travel with it
        valid(
            r#"apple.type = "fruit"
# orange entry

orange.type = "citrus"
apple.skin = "thin"
"#,
            str![[r#"
apple.type = "fruit"
# orange entry

orange.type = "citrus"
apple.skin = "thin"

"#]],
        );
    }

    #[test]
    fn multiline_array_value() {
        // multi-line value is still one statement
        valid(
            r#"apple.colors = [
  "red",
  "green",
]
orange.colors = ["orange"]
apple.type = "fruit"
"#,
            str![[r#"
apple.colors = [
  "red",
  "green",
]
orange.colors = ["orange"]
apple.type = "fruit"

"#]],
        );
    }

    #[test]
    fn inline_table_value() {
        // keys inside an inline table value are not statement starts
        valid(
            r#"apple.info = { type = "fruit", color = "red" }
orange.info = { type = "citrus" }
apple.name = "Apple"
"#,
            str![[r#"
apple.info = { type = "fruit", color = "red" }
orange.info = { type = "citrus" }
apple.name = "Apple"

"#]],
        );
    }

    #[test]
    fn preamble_key_values_sorted() {
        // preamble before the first header is a context too
        valid(
            r#"x.a = 1
y.a = 2
x.b = 3
[section]
"#,
            str![[r#"
x.a = 1
y.a = 2
x.b = 3
[section]

"#]],
        );
    }

    #[test]
    fn single_entry_unchanged() {
        valid(
            r#"foo.bar = 1
"#,
            str![[r#"
foo.bar = 1

"#]],
        );
    }

    #[test]
    fn no_shared_roots_unchanged() {
        valid(
            r#"a.x = 1
b.x = 2
c.x = 3
"#,
            str![[r#"
a.x = 1
b.x = 2
c.x = 3

"#]],
        );
    }

    #[test]
    fn empty_section_body_unchanged() {
        // section with no key-value pairs is a no-op
        valid(
            r#"[a]
[b]
key = 1
"#,
            str![[r#"
[a]
[b]
key = 1

"#]],
        );
    }

    #[test]
    fn array_of_tables_section() {
        valid(
            r#"[[targets]]
foo.x = 1
bar.x = 2
foo.y = 3
"#,
            str![[r#"
[[targets]]
foo.x = 1
bar.x = 2
foo.y = 3

"#]],
        );
    }
}
