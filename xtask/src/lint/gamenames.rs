//! The `game-name` gate.
//!
//! Domain knowledge lives in data. A game id in `smd-rules` — the old
//! `applies_to() -> &["<game id>"]` — is the leak that makes multi-game support a rewrite
//! rather than a file, and this gate is what stops it (ADR-0002 § 5 and § 7).
//!
//! (The literal in that sentence is spelled with a placeholder on purpose: this file is
//! scanned by the gate it implements, and an exemption for the linter's own prose would
//! be the first thing to rot.)
//!
//! Only crates marked `game_names_allowed = true` in `xtask/layers.toml` may contain one;
//! that is `smd-profiles` alone, which holds the knowledge as data. The vocabulary itself
//! lives in `xtask/game-names.toml`, for the obvious reason that a hard-coded list of game
//! names inside the linter would be the first thing the linter has to flag.
//!
//! Doc comments and test fixtures that genuinely need a name go through the same per-file
//! allowance as every other exemption: `// xtask:allow(game-name) reason: …` plus an entry
//! in `xtask/allowances.toml`.

use serde::Deserialize;

use super::deps::LayerMap;
use super::{Ctx, Violation, allowances, rel, rust_sources};

const RULE: &str = "game-name";

#[derive(Deserialize)]
struct Vocabulary {
    names: Vec<String>,
}

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let map = LayerMap::load(ctx)?;
    let path = ctx.root.join("xtask/game-names.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let vocab: Vocabulary =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut out = Vec::new();
    for file in rust_sources(&ctx.root)? {
        let rel_path = rel(&ctx.root, &file);
        let Some(krate) = crate_of(&rel_path) else { continue };
        if map.game_names_allowed(krate) {
            continue;
        }
        let src = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        match allowances::resolve(&ctx.allowances, &rel_path, RULE, &src) {
            Err(v) => out.push(v),
            Ok(true) => {}
            Ok(false) => out.extend(scan(&rel_path, &src, &vocab.names)),
        }
    }
    Ok(out)
}

/// The crate a workspace-relative source path belongs to: `crates/<name>/…`, or the
/// top-level directory for `xtask/…`.
fn crate_of(rel_path: &str) -> Option<&str> {
    let mut parts = rel_path.split('/');
    match parts.next()? {
        "crates" => parts.next(),
        other => Some(other),
    }
}

fn scan(rel_path: &str, src: &str, names: &[String]) -> Vec<Violation> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for name in names {
            if contains_word(&lower, name) {
                out.push(Violation::new(
                    RULE,
                    format!("{rel_path}:{}", i + 1),
                    format!(
                        "game name `{name}`. Game knowledge belongs in `smd-profiles` as \
                         data; a rule declares `requires() -> &[FeatureTag]` instead"
                    ),
                ));
                break;
            }
        }
    }
    out
}

/// Substring match with word boundaries, so a short game id does not fire inside
/// `stack30` and a name does not fire on its own plural.
///
/// `_` is a **boundary**, not a word character: `load_<game>_profile` is precisely the
/// leak this gate exists to catch, and an identifier is the most likely place for one.
fn contains_word(haystack: &str, needle: &str) -> bool {
    let is_word = char::is_alphanumeric;
    let mut from = 0;
    while let Some(found) = haystack[from..].find(needle) {
        let start = from + found;
        let end = start + needle.len();
        let before_ok = start == 0 || !haystack[..start].chars().next_back().is_some_and(is_word);
        let after_ok = end == haystack.len() || !haystack[end..].starts_with(is_word);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

#[cfg(test)]
mod tests;
