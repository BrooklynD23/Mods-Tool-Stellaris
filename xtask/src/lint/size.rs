//! The `file-length` gate: 300 code lines, hard fail; 200, warn.
//!
//! A file past 300 lines is two concepts. Function length and parameter count are clippy's
//! job (`too_many_lines` at 60, `too_many_arguments` at 5); this gate is the one with no
//! clippy equivalent.

use super::{Ctx, Violation, allowances, rel, rust_sources};
use crate::codelines::{code_lines, is_all_const_or_static};

const HARD_LIMIT: usize = 300;
const WARN_LIMIT: usize = 200;
const RULE: &str = "file-length";

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let mut out = Vec::new();
    for path in rust_sources(&ctx.root)? {
        let rel_path = rel(&ctx.root, &path);
        if rel_path.contains("/generated/") {
            continue;
        }
        let src = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let lines = code_lines(&src);

        if lines.len() <= WARN_LIMIT {
            continue;
        }
        if is_all_const_or_static(&lines) {
            continue;
        }
        match allowances::resolve(&ctx.allowances, &rel_path, RULE, &src) {
            Err(v) => out.push(v),
            Ok(true) => {}
            Ok(false) if lines.len() > HARD_LIMIT => {
                out.push(Violation::new(
                    RULE,
                    format!("{rel_path}:{}", lines[HARD_LIMIT].number),
                    format!(
                        "{} code lines, limit {HARD_LIMIT}. Split it, or add \
                         `// xtask:allow({RULE}) reason: …` plus an entry in \
                         xtask/allowances.toml",
                        lines.len()
                    ),
                ));
            }
            Ok(false) => {
                println!(
                    "warning[{RULE}]: {rel_path} is {} code lines (warn at {WARN_LIMIT}, \
                     hard fail at {HARD_LIMIT})",
                    lines.len()
                );
            }
        }
    }
    Ok(out)
}
