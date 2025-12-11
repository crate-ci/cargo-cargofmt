#[tracing::instrument]
pub fn normalize_indent(
    _tokens: &mut crate::toml::TomlTokens<'_>,
    _hard_tabs: bool,
    _tab_spaces: usize,
) {
}

#[cfg(test)]
mod test {
    use snapbox::assert_data_eq;
    use snapbox::str;
    use snapbox::IntoData;

    #[track_caller]
    fn valid(input: &str, hard_tabs: bool, tab_spaces: usize, expected: impl IntoData) {
        let mut tokens = crate::toml::TomlTokens::parse(input);
        super::normalize_indent(&mut tokens, hard_tabs, tab_spaces);
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
    fn empty_tabs() {
        valid("", true, 10, str![]);
    }

    #[test]
    fn empty_spaces() {
        valid("", false, 10, str![]);
    }

    #[test]
    fn cleanup_tabs() {
        valid(
            "
  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11
",
            true,
            10,
            str![[r#"

  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11

"#]],
        );
    }

    #[test]
    fn cleanup_spaces() {
        valid(
            "
  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11
",
            false,
            10,
            str![[r#"

  a = 5

  # Hello

  [b]
  a = 10
  b = [
    1,
    2,
    3,
  ]
  c = [
    [
      1,
      2,
      3,
    ]
  ]
  d = [[
      1,
      2,
      3,
  ]]

  [e]
    f = 10

g = 11

"#]],
        );
    }
}
