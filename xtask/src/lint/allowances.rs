//! The per-file exemption ratchet.
//!
//! An exemption is valid only when **both** halves are present: an inline
//! `// xtask:allow(<rule>) reason: …` in the file, and an entry in
//! `xtask/allowances.toml`. Either half alone is itself a violation, so an exemption
//! cannot outlive the code that needed it, and adding one is visible in the diff of a
//! data file rather than buried in a source comment.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use super::Violation;

#[derive(Deserialize)]
struct File {
    #[serde(default)]
    allowance: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    file: String,
    rule: String,
    reason: String,
}

/// The loaded ratchet: the set of `(file, rule)` pairs the TOML permits.
pub struct Allowances {
    declared: BTreeSet<(String, String)>,
}

impl Allowances {
    pub fn load(root: &Path) -> Result<Allowances, String> {
        let path = root.join("xtask/allowances.toml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let parsed: File = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

        let mut declared = BTreeSet::new();
        for e in parsed.allowance {
            if e.reason.trim().is_empty() {
                return Err(format!(
                    "xtask/allowances.toml: entry for {} rule {} has an empty reason",
                    e.file, e.rule
                ));
            }
            declared.insert((e.file, e.rule));
        }
        Ok(Allowances { declared })
    }

    /// Whether `xtask/allowances.toml` records this exemption.
    pub fn declared(&self, file: &str, rule: &str) -> bool {
        self.declared.contains(&(file.to_owned(), rule.to_owned()))
    }
}

/// Whether a file carries the inline half of the exemption for `rule`.
///
/// The marker must be an ordinary `//` comment. A `///` or `//!` doc comment does not
/// count — otherwise this module's own documentation, which has to spell the marker out,
/// would exempt the linter from its own gates.
pub fn inline_allow(src: &str, rule: &str) -> bool {
    let needle = format!("xtask:allow({rule})");
    src.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with("//")
            && !t.starts_with("///")
            && !t.starts_with("//!")
            && t.contains(&needle)
            && t.contains("reason:")
    })
}

/// Resolve an exemption, returning the violation that stands in its way, if any.
///
/// - both halves present → `Ok(true)`, the gate is silenced for this file;
/// - neither present → `Ok(false)`, the gate applies normally;
/// - exactly one present → `Err`, because a half-exemption is a stale exemption.
pub fn resolve(
    allowances: &Allowances,
    rel_path: &str,
    rule: &'static str,
    src: &str,
) -> Result<bool, Violation> {
    match (inline_allow(src, rule), allowances.declared(rel_path, rule)) {
        (true, true) => Ok(true),
        (false, false) => Ok(false),
        (true, false) => Err(Violation::new(
            rule,
            rel_path.to_owned(),
            format!(
                "file carries `// xtask:allow({rule}) reason: …` with no matching entry in \
                 xtask/allowances.toml"
            ),
        )),
        (false, true) => Err(Violation::new(
            rule,
            "xtask/allowances.toml".to_owned(),
            format!(
                "stale allowance: {rel_path} is listed for `{rule}` but no longer carries \
                 `// xtask:allow({rule}) reason: …`"
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inline_allow_needs_both_the_rule_and_a_reason() {
        assert!(inline_allow("// xtask:allow(file-length) reason: table\n", "file-length"));
        assert!(!inline_allow("// xtask:allow(file-length)\n", "file-length"));
        assert!(!inline_allow("//! xtask:allow(file-length) reason: prose\n", "file-length"));
        assert!(!inline_allow("/// xtask:allow(file-length) reason: prose\n", "file-length"));
        assert!(!inline_allow("// xtask:allow(game-name) reason: x\n", "file-length"));
        assert!(!inline_allow("let s = \"xtask:allow(file-length) reason:\";\n", "file-length"));
    }

    #[test]
    fn half_an_exemption_is_a_violation_on_either_side() {
        let empty = Allowances { declared: BTreeSet::new() };
        let src = "// xtask:allow(file-length) reason: because\n";
        assert!(resolve(&empty, "a.rs", "file-length", src).is_err());
        assert!(!resolve(&empty, "a.rs", "file-length", "").unwrap());

        let mut declared = BTreeSet::new();
        declared.insert(("a.rs".to_owned(), "file-length".to_owned()));
        let listed = Allowances { declared };
        assert!(resolve(&listed, "a.rs", "file-length", "").is_err());
        assert!(resolve(&listed, "a.rs", "file-length", src).unwrap());
    }
}
