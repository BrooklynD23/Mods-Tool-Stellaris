//! The `doc-link` gate: relative links in `docs/**/*.md` must resolve.
//!
//! Four to five broken cross-references were among the cross-confirmed findings of the
//! August review (§ G29). Documents that cite each other by path are load-bearing here —
//! the review register itself is cited from six other files — and a link checker is the
//! cheapest gate in the set.

use std::path::{Path, PathBuf};

use super::{Ctx, Violation, collect, rel};

const RULE: &str = "doc-link";

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let mut files = Vec::new();
    collect(&ctx.root.join("docs"), "md", &mut files)?;
    files.sort();

    let mut out = Vec::new();
    for file in files {
        let src = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let dir = file.parent().unwrap_or(&ctx.root).to_path_buf();
        for (line, target) in links(&src) {
            if let Some(detail) = broken(&dir, &target) {
                out.push(Violation::new(RULE, format!("{}:{line}", rel(&ctx.root, &file)), detail));
            }
        }
    }
    Ok(out)
}

/// Why a link target does not resolve, or `None` if it does.
fn broken(dir: &Path, target: &str) -> Option<String> {
    if target.is_empty() || target.starts_with('#') || is_absolute_url(target) {
        return None;
    }
    let path = target.split('#').next().unwrap_or(target);
    if path.is_empty() {
        return None;
    }
    let resolved: PathBuf = dir.join(path);
    if resolved.exists() {
        None
    } else {
        Some(format!("broken relative link `{target}` -> {}", resolved.display()))
    }
}

fn is_absolute_url(target: &str) -> bool {
    const SCHEMES: [&str; 5] = ["http://", "https://", "mailto:", "tel:", "ftp://"];
    SCHEMES.iter().any(|s| target.starts_with(s)) || target.starts_with('/')
}

/// Every `[text](target)` link, as `(line number, target)`.
///
/// Deliberately not a markdown parser: the gate needs to be wrong in the safe direction,
/// and a fenced code block containing `](` yields a target that either resolves or is
/// worth looking at anyway.
fn links(src: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (i, line) in src.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let mut rest = line;
        while let Some(open) = rest.find("](") {
            let after = &rest[open + 2..];
            match after.find(')') {
                Some(close) => {
                    out.push((i + 1, after[..close].trim().to_owned()));
                    rest = &after[close + 1..];
                }
                None => break,
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_found_outside_fenced_blocks_only() {
        let src = "see [a](x.md) and [b](y.md#h)\n```\n[c](z.md)\n```\n[d](w.md)\n";
        let found = links(src);
        assert_eq!(
            found,
            vec![(1, "x.md".to_owned()), (1, "y.md#h".to_owned()), (5, "w.md".to_owned()),]
        );
    }

    #[test]
    fn absolute_and_anchor_targets_are_not_checked() {
        let dir = Path::new("/nonexistent");
        assert!(broken(dir, "https://example.com").is_none());
        assert!(broken(dir, "#section").is_none());
        assert!(broken(dir, "mailto:a@b.c").is_none());
        assert!(broken(dir, "missing.md").is_some());
    }
}
