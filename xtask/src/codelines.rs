//! Counting **code** lines, not raw lines.
//!
//! Blank lines, `//`/`///`/`//!` comments, `/* … */` blocks, and `#[cfg(test)]` items do
//! not count. Raw counting taxes documentation and creates an incentive to delete the most
//! valuable text in the file — and this codebase is doc-comment-heavy by design.
//! (ADR-0002 § 7, doc 02 § Size discipline.)

/// One code line, with its 1-based line number in the original file.
pub struct CodeLine {
    /// 1-based line number in the original file.
    pub number: usize,
    /// The line's text, with the trailing newline removed.
    pub text: String,
}

/// Strip everything that does not count towards the file-length gate.
pub fn code_lines(src: &str) -> Vec<CodeLine> {
    let mut out = Vec::new();
    let mut in_block_comment = 0usize;
    let mut test_depth: Option<i32> = None;

    for (i, raw) in src.lines().enumerate() {
        let line = strip_block_comments(raw, &mut in_block_comment);
        let trimmed = line.trim();

        if let Some(depth) = test_depth.as_mut() {
            *depth += brace_delta(trimmed);
            if *depth <= 0 {
                test_depth = None;
            }
            continue;
        }
        if trimmed.starts_with("#[cfg(test)]") || trimmed.starts_with("#[cfg(all(test") {
            // `mod tests;` on the following line is one line; `mod tests { … }` is a block.
            test_depth = Some(0);
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        out.push(CodeLine { number: i + 1, text: line });
    }
    out
}

/// Net brace balance of a line, ignoring braces inside string and char literals.
fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    let mut in_str = false;
    let mut escaped = false;
    for c in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' if in_str => escaped = true,
            '"' => in_str = !in_str,
            '{' if !in_str => delta += 1,
            '}' if !in_str => delta -= 1,
            _ => {}
        }
    }
    delta
}

/// Remove `/* … */` spans, tracking nesting across lines.
fn strip_block_comments(line: &str, depth: &mut usize) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            *depth += 1;
            i += 2;
        } else if bytes[i..].starts_with(b"*/") && *depth > 0 {
            *depth -= 1;
            i += 2;
        } else {
            if *depth == 0 {
                out.push(bytes[i] as char);
            }
            i += 1;
        }
    }
    if line.is_ascii() { out } else { fallback_non_ascii(line, *depth, out) }
}

/// The byte-wise fast path above mangles multi-byte characters, which only matters for a
/// line that is not pure ASCII. Those lines are rare and never contain a brace we care
/// about, so keep them whole when no comment is open.
fn fallback_non_ascii(line: &str, depth: usize, ascii_out: String) -> String {
    if depth == 0 && !line.contains("/*") && !line.contains("*/") {
        line.to_owned()
    } else {
        ascii_out
    }
}

/// Whether every top-level item in the file is a `const` or `static`.
///
/// Exempt by construction: the lexer's byte-class tables, the Windows-1252 mapping, and
/// the golden classification lists are data that happens to be written in Rust, and
/// splitting them across four files makes them harder to review, not easier.
pub fn is_all_const_or_static(lines: &[CodeLine]) -> bool {
    const ITEM_STARTS: [&str; 8] =
        ["fn ", "impl ", "struct ", "enum ", "trait ", "mod ", "macro_rules!", "type "];

    let mut saw_data = false;
    for l in lines {
        if l.text.starts_with(char::is_whitespace) {
            continue;
        }
        let t = l.text.trim_start_matches("pub ").trim_start_matches("pub(crate) ");
        if ITEM_STARTS.iter().any(|s| t.starts_with(s)) {
            return false;
        }
        if t.starts_with("const ") || t.starts_with("static ") {
            saw_data = true;
        }
    }
    saw_data
}

#[cfg(test)]
mod tests;
