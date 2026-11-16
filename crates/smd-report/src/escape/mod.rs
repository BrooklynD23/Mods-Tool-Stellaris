//! Sink escaping (doc 11, Output safety). One module per sink; nothing else in the crate
//! escapes anything, and no call site formats data into a sink without going through one.
//!
//! This module holds the one thing the terminal and Markdown sinks share: control
//! characters and bidirectional formatting characters become visible `\u{..}` text.
//! JSON has its own module for the characters `serde_json` leaves raw.

pub(crate) mod json;
pub(crate) mod markdown;
pub(crate) mod terminal;

use std::fmt::Write as _;

/// Whether `c` must never reach a terminal or a Markdown document as itself.
///
/// C0 and C1 controls (and DEL), the bidirectional overrides and isolates U+202A..U+202E
/// and U+2066..U+2069 ("Trojan Source"), the directional marks U+061C, U+200E, U+200F, and
/// the line and paragraph separators U+2028, U+2029, and invisible format characters that
/// can spoof a name or path: soft hyphen, U+180E, zero-width U+200B..U+200D, the word
/// joiner and invisible operators U+2060..U+2064, U+FEFF, the interlinear annotation marks
/// U+FFF9..U+FFFB, and the Tag block U+E0000..U+E007F.
pub(crate) fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00ad}'
                | '\u{061c}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{e0000}'..='\u{e007f}'
        )
}

/// Append `c` as a visible `\u{..}` escape.
pub(crate) fn push_escaped(out: &mut String, c: char) {
    // Writing to a `String` cannot fail.
    let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
}

/// Replace every unsafe character in `s` with its visible escape.
pub(crate) fn escape_controls(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if is_unsafe(c) {
            push_escaped(&mut out, c);
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
    fn controls_and_bidi_become_visible_text() {
        let s = escape_controls("a\u{1b}b\u{7}c\u{202e}d\u{2066}e\u{85}f\ng\u{7f}");
        assert_eq!(s, "a\\u{1b}b\\u{7}c\\u{202e}d\\u{2066}e\\u{85}f\\u{a}g\\u{7f}");
        assert!(s.is_ascii());
    }

    #[test]
    fn invisible_format_characters_become_visible_text() {
        let s = escape_controls("a\u{200b}b\u{feff}c\u{2060}d\u{e0041}e\u{ad}");
        assert_eq!(s, "a\\u{200b}b\\u{feff}c\\u{2060}d\\u{e0041}e\\u{ad}");
    }

    #[test]
    fn ordinary_text_including_non_ascii_is_untouched() {
        assert_eq!(escape_controls("Zażółć gęślą jaźń — 漢字"), "Zażółć gęślą jaźń — 漢字");
    }
}
