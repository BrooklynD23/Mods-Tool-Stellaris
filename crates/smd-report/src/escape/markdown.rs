//! The Markdown sink (also the clipboard summary's, when it exists).
//!
//! Data never becomes markup: metacharacters are backslash-escaped, `<`, `>` and `&` become
//! entities so that no raw HTML can pass, `:` and `@` are escaped so that no autolink forms,
//! and code spans are fenced with more backticks than the content holds. No link is ever
//! generated from data.

use super::escape_controls;

/// ASCII punctuation that is backslash-escaped in running text. Together with the three
/// entity characters this covers every character CommonMark or GFM can read as syntax or
/// as the start of an autolink.
const BACKSLASH_ESCAPED: &str = "\\`*_{}[]()#+-.!|~:@$";

/// Escape one data string for running Markdown text (a paragraph, list item, or table cell).
pub(crate) fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in escape_controls(s).chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            c if BACKSLASH_ESCAPED.contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out
}

/// Render a mod display name: escaped text, except that a name holding a backtick becomes
/// a fenced code span, so a run of backticks can never open or close anything around it.
pub(crate) fn name(s: &str) -> String {
    if s.contains('`') { code_span(s) } else { text(s) }
}

/// A value from a closed vocabulary (`not_applicable`, `workshop`): written as it is when it
/// is lowercase ASCII, digits, and underscores, which no Markdown reads as syntax, and
/// escaped like any other data otherwise.
pub(crate) fn tag(s: &str) -> String {
    let plain = !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if plain { s.to_owned() } else { text(s) }
}

/// Render `s` as an inline code span, fenced with one more backtick than the longest run
/// in the content and padded with a space where the content touches a backtick.
///
/// `|` is written `\|`, which GFM reads as a literal pipe even inside a code span in a
/// table cell.
pub(crate) fn code_span(s: &str) -> String {
    let body = escape_controls(s).replace('|', "\\|");
    let fence = "`".repeat(longest_backtick_run(&body) + 1);
    let pad = if body.starts_with('`') || body.ends_with('`') { " " } else { "" };
    format!("{fence}{pad}{body}{pad}{fence}")
}

fn longest_backtick_run(s: &str) -> usize {
    s.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_arrives_as_characters() {
        let out = text("[Fix](https://e.x)");
        assert!(!out.contains("]("));
        assert_eq!(out, "\\[Fix\\]\\(https\\://e\\.x\\)");
    }

    #[test]
    fn vocabulary_tags_stay_readable_and_anything_else_is_escaped() {
        assert_eq!(tag("not_applicable"), "not_applicable");
        assert_eq!(tag("a*b"), "a\\*b");
        assert_eq!(tag(""), "");
    }

    #[test]
    fn dollar_and_backtick_names_are_inert() {
        assert_eq!(text("$x$"), "\\$x\\$");
        let out = name("a`````b");
        assert!(out.starts_with("``````a") && out.ends_with("b``````"));
        assert_eq!(name("[Fix](u)"), text("[Fix](u)"));
    }

    #[test]
    fn html_cannot_pass() {
        let out = text("<script>alert(1)</script> & <b>");
        assert!(!out.contains('<') && !out.contains('>'));
        assert!(out.contains("&lt;script&gt;") && out.contains("&amp;"));
    }

    #[test]
    fn the_fence_outgrows_the_content() {
        let out = code_span("a`````b");
        assert!(out.starts_with("``````a") && out.ends_with("b``````"));
        assert_eq!(code_span("`x"), "`` `x ``");
        assert_eq!(code_span("plain"), "`plain`");
    }

    #[test]
    fn controls_are_visible_in_both_forms() {
        assert_eq!(text("a\u{1b}"), "a\\\\u\\{1b\\}");
        assert!(code_span("a\u{202e}").contains("\\u{202e}"));
    }
}
