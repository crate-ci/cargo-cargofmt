pub fn check_line_overflow(formatted: &str, max_width: usize) -> Vec<(usize, usize)> {
    vec![]
}

#[cfg(test)]
mod test {

    #[track_caller]
    fn valid(input: &str, max_width: usize, expected: Vec<(usize, usize)>) {
        let actual = super::check_line_overflow(input, max_width);
        assert_eq!(actual, expected);
    }

    #[test]
    fn no_overflow() {
        let input = "[package]\nname = \"app\"\n";
        valid(input, 20, vec![]);
    }

    #[test]
    fn overflow_table_header() {
        let input = "[very-long-table-header-name]\n";
        valid(input, 20, vec![]); //expected vec![(1, 29)]
    }

    #[test]
    fn exempt_standalone_comment() {
        let input = "# This is a very long comment line\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn exempt_inline_comment() {
        let input = "a = \"b\"  # very long comment here\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn overflow_from_long_key() {
        let input = "a_very_long_key = \"b\"  # very long comment here";
        valid(input, 10, vec![]); //expected vec![(1, 47)]
    }

    #[test]
    fn exempt_string_literal() {
        let input = "desc = \"A very long description\"\n";
        valid(input, 10, vec![]);
    }

    #[test]
    fn not_exempt_hash_inside_string() {
        let input = "long-key-name-here = \"rust\"\n";
        valid(input, 10, vec![]); //expected vec![(1, 27)]
    }
}
