//! Text normalization for search (roadmap 2.4): user-entered data is never
//! translated or rewritten, but a search must ignore the writing variants
//! that are common in everyday Dari/Pashto typing — different letterforms,
//! joiner characters, and digit scripts. Not wired into a search feature yet
//! (Patient Search is Phase 3); this is the shared, tested building block
//! both that search and any future one should call.

use crate::calendar::digits;

/// Folds a string to a search-friendly key: Arabic ی/ك become Persian ی/ک,
/// ZWNJ and the various "half-space" characters collapse to a plain space,
/// digits become Latin, and the result is lowercased and trimmed of repeated
/// whitespace. Two strings that a person would consider "the same name" with
/// slightly different typing normalize to the same key.
pub fn normalize_for_search(s: &str) -> String {
    let folded: String = s
        .chars()
        .map(|c| match c {
            // Arabic yeh/kaf → Persian yeh/keh (common typing inconsistency).
            '\u{064A}' => '\u{06CC}', // ي → ی
            '\u{0643}' => '\u{06A9}', // ك → ک
            // ZWNJ (half-space) and Arabic presentation of tatweel collapse to space.
            '\u{200C}' | '\u{200D}' | '\u{0640}' => ' ',
            _ => c,
        })
        .collect();
    let latin_digits = digits::to_latin(&folded);
    let collapsed = latin_digits.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_common_letter_variants() {
        assert_eq!(normalize_for_search("علی"), normalize_for_search("علي"));
        assert_eq!(normalize_for_search("کابل"), normalize_for_search("كابل"));
    }

    #[test]
    fn collapses_joiners_and_spacing() {
        assert_eq!(normalize_for_search("احمد\u{200C}خان"), normalize_for_search("احمد خان"));
        assert_eq!(normalize_for_search("احمد   خان"), normalize_for_search("احمد خان"));
    }

    #[test]
    fn folds_digit_scripts() {
        assert_eq!(normalize_for_search("کلینیک ۱۲۳"), normalize_for_search("کلینیک 123"));
    }

    #[test]
    fn is_case_insensitive_for_latin_text() {
        assert_eq!(normalize_for_search("Ahmad Khan"), normalize_for_search("ahmad khan"));
    }
}
