//! Text put into a CSV file or onto the clipboard as a table: the one
//! field quoter every table writer uses, so a watch named "Pepsi, 1972"
//! or a note holding a quote stays in its own column.

/// `s` as one field of a line split by `sep` (a comma for a CSV file, a
/// tab for the clipboard): quoted where it holds `sep`, a double quote or
/// a line break, with each double quote doubled inside the quotes
/// (RFC 4180); otherwise as it is.
pub fn field(s: &str, sep: char) -> String {
    if s.contains([sep, '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_is_quoted_only_where_it_must_be() {
        assert_eq!(field("Pepsi, 1972", ','), "\"Pepsi, 1972\"");
        assert_eq!(field("Pepsi, 1972", '\t'), "Pepsi, 1972");
        assert_eq!(field("a\tb", '\t'), "\"a\tb\"");
        assert_eq!(field("the \"Pepsi\"", '\t'), "\"the \"\"Pepsi\"\"\"");
        assert_eq!(field("two\nlines", ','), "\"two\nlines\"");
        assert_eq!(field("two\rlines", '\t'), "\"two\rlines\"");
        assert_eq!(field("Kew–Teddington 90°", ','), "Kew–Teddington 90°");
        assert_eq!(field("", ','), "");
    }
}
