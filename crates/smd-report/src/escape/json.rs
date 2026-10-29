//! The JSON sink. `serde_json` escapes only below U+0020, so C1 controls (U+009B is CSI),
//! bidi and invisible characters, and U+2028/2029 would reach a terminal raw when the JSON
//! is printed there. They are rewritten as `\uXXXX` escapes, which is still valid JSON and
//! round-trips to the same string.

use std::fmt::Write as _;

use super::is_unsafe;

/// Rewrite every unsafe non-ASCII character of serialised JSON as a `\u` escape (a
/// surrogate pair above the BMP). Outside strings the serialised text is ASCII, so this
/// only ever touches string contents. ASCII controls were already escaped by `serde_json`.
pub(crate) fn harden(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if !c.is_ascii() && is_unsafe(c) {
            let mut units = [0u16; 2];
            for u in c.encode_utf16(&mut units) {
                // Writing to a `String` cannot fail.
                let _ = write!(out, "\\u{u:04x}");
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_characters_are_escaped_and_round_trip() {
        let original = "a\u{9b}b\u{202e}c\u{2028}d\u{e0041}e\u{200b}f 漢";
        let json = harden(&serde_json::to_string(original).expect("ser"));
        assert!(json.is_ascii() || json.contains('漢'));
        assert!(!json.contains('\u{9b}') && !json.contains('\u{202e}'));
        assert!(!json.contains('\u{e0041}') && !json.contains('\u{2028}'));
        let back: String = serde_json::from_str(&json).expect("de");
        assert_eq!(back, original);
    }
}
