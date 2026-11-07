//! Windows CRT command-line quoting, including embedded quotes and trailing slashes.
pub fn quote_argument(argument: &[u16]) -> Vec<u16> {
    let mut output = vec![34];
    let mut slashes = 0;
    for &unit in argument {
        if unit == 92 {
            slashes += 1;
            continue;
        }
        output.extend(std::iter::repeat_n(
            92,
            if unit == 34 { slashes * 2 + 1 } else { slashes },
        ));
        output.push(unit);
        slashes = 0;
    }
    output.extend(std::iter::repeat_n(92, slashes * 2));
    output.push(34);
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    fn quote(s: &str) -> String {
        String::from_utf16(&quote_argument(&s.encode_utf16().collect::<Vec<_>>())).unwrap()
    }
    #[test]
    fn preserves_empty_unicode_quotes_and_trailing_backslashes() {
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("D:\\中文 路径\\"), "\"D:\\中文 路径\\\\\"");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote("a\\\"b"), "\"a\\\\\\\"b\"");
        assert_eq!(quote("--text"), "\"--text\"");
    }
}
