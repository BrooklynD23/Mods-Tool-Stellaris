//! The terminal sink: data is printed inert, colour comes only from the tool.

use super::escape_controls;

/// Escape one data string for a terminal cell or line.
pub(crate) fn text(s: &str) -> String {
    escape_controls(s)
}

/// Wrap tool-authored `s` in an ANSI style when `color` is on. `s` must already be
/// escaped data or tool text, and `code` is one of this module's constants.
pub(crate) fn styled(s: &str, code: &str, color: bool) -> String {
    if color { format!("\u{1b}[{code}m{s}\u{1b}[0m") } else { s.to_owned() }
}

/// Bold.
pub(crate) const BOLD: &str = "1";
/// Yellow, for the incomplete-scan warning.
pub(crate) const YELLOW: &str = "33";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hyperlink_forgery_is_inert() {
        let out = text("\u{1b}]8;;http://e.x\u{7}Evil\u{1b}]8;;\u{7}");
        assert!(!out.contains('\u{1b}') && !out.contains('\u{7}'));
        assert!(out.contains("\\u{1b}]8;;http://e.x\\u{7}Evil"));
    }

    #[test]
    fn colour_is_off_unless_asked_for() {
        assert_eq!(styled("x", BOLD, false), "x");
        assert_eq!(styled("x", BOLD, true), "\u{1b}[1mx\u{1b}[0m");
    }
}
