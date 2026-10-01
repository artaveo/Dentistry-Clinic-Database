//! ADR-09: store/search with Latin digits, display per user setting,
//! accept any digit script on input.

/// Converts Persian (U+06F0..) and Arabic-Indic (U+0660..) digits to ASCII.
/// Apply to every user input before validation, search or storage.
pub fn to_latin(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            _ => c,
        })
        .collect()
}

/// Latin → Extended Arabic-Indic (Persian) digits, used for Dari and Pashto display.
pub fn to_persian(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '0'..='9' => char::from_u32(0x06F0 + (c as u32 - '0' as u32)).unwrap(),
            _ => c,
        })
        .collect()
}

/// Search key for phone numbers: Latin digits only, so `۰۷۰۰ ۱۲۳ ۴۵۶`,
/// `0700-123-456` and `0700123456` all match.
pub fn phone_search_key(s: &str) -> String {
    to_latin(s).chars().filter(|c| c.is_ascii_digit()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_all_digit_scripts() {
        assert_eq!(to_latin("۰۷۹۹ ١٢٣ 456"), "0799 123 456");
        assert_eq!(to_persian("P-000123"), "P-۰۰۰۱۲۳");
        assert_eq!(phone_search_key("۰۷۰۰-۱۲۳ ۴۵۶"), phone_search_key("0700123456"));
    }
}
