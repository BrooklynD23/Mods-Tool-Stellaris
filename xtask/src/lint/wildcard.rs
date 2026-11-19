//! The `shared-enum-wildcard` gate (ADR-0002 § 3; REVIEW-2026-10 H36).
//!
//! Outside `smd-core`, a `match` whose arms name a shared `smd-core` enum may not end in
//! `_ => {}`, `_ => ()` or `_ => None`: the wildcard a `#[non_exhaustive]` enum forces must
//! degrade loudly, never into silence. A shared enum is any `pub enum` in `smd-core`.
//!
//! ponytail: text-level, not type-level. It finds the enclosing `match` by brace balance
//! and recognises the scrutinee's enum by `Enum::` in the arm patterns, so a match over a
//! shared enum that names no variant before the wildcard (only bindings or literals) is
//! missed. A `syn`-based visitor closes that when it is worth a dependency.

use super::{Ctx, Violation, allowances, rel, rust_sources};
use crate::codelines::{CodeLine, code_lines};

const RULE: &str = "shared-enum-wildcard";
const EMPTY_ARMS: [&str; 6] =
    ["_ => {}", "_ => {},", "_ => ()", "_ => (),", "_ => None", "_ => None,"];

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let shared = shared_enums(ctx)?;
    let mut out = Vec::new();
    for file in rust_sources(&ctx.root)? {
        let rel_path = rel(&ctx.root, &file);
        if rel_path.starts_with("crates/smd-core/") {
            continue;
        }
        let src = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        match allowances::resolve(&ctx.allowances, &rel_path, RULE, &src) {
            Err(v) => out.push(v),
            Ok(true) => {}
            Ok(false) => out.extend(scan(&rel_path, &code_lines(&src), &shared)),
        }
    }
    Ok(out)
}

/// Every `pub enum` name declared in `crates/smd-core/src`.
fn shared_enums(ctx: &Ctx) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    super::collect(&ctx.root.join("crates/smd-core/src"), "rs", &mut files)?;
    let mut names = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        for line in src.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("pub enum ") {
                let name: String =
                    rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                names.push(name);
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}

fn scan(rel_path: &str, lines: &[CodeLine], shared: &[String]) -> Vec<Violation> {
    let mut out = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if !EMPTY_ARMS.contains(&l.text.trim()) {
            continue;
        }
        if let Some(name) = matched_enum(&lines[..i], shared) {
            out.push(Violation::new(
                RULE,
                format!("{rel_path}:{}", l.number),
                format!(
                    "empty wildcard arm over the shared enum `{name}`. A new variant would \
                     vanish here silently; handle it or emit a diagnostic (ADR-0002 § 3)"
                ),
            ));
        }
    }
    out
}

/// The shared enum named by the arm patterns of the `match` enclosing the line after
/// `before`, if that block is a `match`.
fn matched_enum<'a>(before: &[CodeLine], shared: &'a [String]) -> Option<&'a str> {
    let mut depth = 0;
    for (j, l) in before.iter().enumerate().rev() {
        depth += brace_delta(&l.text);
        if depth > 0 {
            if !l.text.contains("match ") {
                return None;
            }
            let arms = before[j + 1..].iter().filter_map(|a| a.text.split_once("=>"));
            return arms
                .flat_map(|(pat, _)| shared.iter().filter(move |n| names(pat, n)))
                .map(String::as_str)
                .next();
        }
    }
    None
}

/// Whether `pat` mentions `name::` as a whole path segment.
fn names(pat: &str, name: &str) -> bool {
    let needle = format!("{name}::");
    pat.match_indices(&needle).any(|(at, _)| {
        !pat[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Net brace balance, ignoring braces in string and char literals.
fn brace_delta(line: &str) -> i32 {
    let mut delta = 0;
    let (mut in_str, mut escaped) = (false, false);
    let chars: Vec<char> = line.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if escaped {
            escaped = false;
        } else if in_str {
            match c {
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '\'' && chars.get(k + 2) == Some(&'\'') {
            k += 2; // a one-character literal such as '{'
        } else if c == '{' {
            delta += 1;
        } else if c == '}' {
            delta -= 1;
        }
        k += 1;
    }
    delta
}
