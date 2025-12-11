#[tracing::instrument]
pub fn constrain_blank_lines(_tokens: &mut crate::toml::TomlTokens<'_>, _min: usize, _max: usize) {}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, min: usize, max: usize, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::constrain_blank_lines(&mut tokens, min, max);
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
    fn empty_no_blank() {
        valid("", 0, 0, str![]);
    }

    #[test]
    fn empty_many_blanks() {
        valid("", 3, 10, str![]);
    }

    #[test]
    fn single_key_value_no_blank() {
        valid("a = 5", 0, 0, str!["a = 5"]);
    }

    #[test]
    fn single_key_value_many_blanks() {
        valid("a = 5", 3, 10, str!["a = 5"]);
    }

    #[test]
    fn remove_blank_lines() {
        valid(
            "

a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,

]


g = { a = 1, b = 2 }


",
            0,
            0,
            str![[r#"


a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,

]


g = { a = 1, b = 2 }



"#]],
        );
    }

    #[test]
    fn compact_blank_lines() {
        valid(
            "

a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,


]


g = { a = 1, b = 2 }


",
            1,
            1,
            str![[r#"


a = 5


b = 6


# comment 


# comment


c = 7


[d]


e = 10


f = [


  1,


  2,


]


g = { a = 1, b = 2 }



"#]],
        );
    }

    #[test]
    fn add_blank_lines() {
        valid(
            "a = 5
b = 6
# comment 
# comment
c = 7
[d]
e = 10
f = [
  1,
  2,
]
g = { a = 1, b = 2 }",
            2,
            2,
            str![[r#"
a = 5
b = 6
# comment 
# comment
c = 7
[d]
e = 10
f = [
  1,
  2,
]
g = { a = 1, b = 2 }
"#]],
        );
    }

    #[test]
    fn expand_blank_lines() {
        valid(
            "
a = 5

b = 6

# comment 

# comment

c = 7

[d]

e = 10

f = [

  1,

  2,

]

g = { a = 1, b = 2 }
",
            3,
            3,
            str![[r#"

a = 5

b = 6

# comment 

# comment

c = 7

[d]

e = 10

f = [

  1,

  2,

]

g = { a = 1, b = 2 }

"#]],
        );
    }
}
