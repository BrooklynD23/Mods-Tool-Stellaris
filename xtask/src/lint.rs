//! `cargo xtask lint` — the structural gates.
//!
//! Five gates, each failing the build:
//!
//! | Rule | What it fails on |
//! | --- | --- |
//! | `file-length` | a file over 300 **code** lines |
//! | `dep-direction` | an edge that contradicts `xtask/layers.toml`, checked against `cargo metadata` |
//! | `game-name` | a game name in a crate without `game_names_allowed` |
//! | `doc-link` | a broken relative link in `docs/**/*.md` |
//! | `profile-audit` | `[profile.release]` or the delegated clippy gates drifting |
//!
//! Function length (60 lines) and parameter count (5) are delegated to clippy, whose
//! thresholds live in `clippy.toml`; `profile-audit` checks that the delegation is still
//! wired up. `clippy::cognitive_complexity` is deliberately absent — it is a nursery lint
//! that misfires and moves between toolchain releases, so a routine `rustup update` would
//! turn CI red on code nobody touched.

mod allowances;
mod deps;
mod doclinks;
mod gamenames;
mod profile;
mod size;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// One gate failure.
#[derive(Debug)]
pub struct Violation {
    /// The gate that fired.
    pub rule: &'static str,
    /// Where, as a workspace-relative path plus an optional line.
    pub at: String,
    /// What is wrong, and what to do about it.
    pub detail: String,
}

impl Violation {
    fn new(rule: &'static str, at: impl Into<String>, detail: impl Into<String>) -> Violation {
        Violation { rule, at: at.into(), detail: detail.into() }
    }
}

/// Everything a gate needs: where the workspace is, and the two data files.
pub struct Ctx {
    /// Workspace root.
    pub root: PathBuf,
    /// The per-file exemption ratchet.
    pub allowances: allowances::Allowances,
}

pub fn run() -> ExitCode {
    let root = match workspace_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("xtask lint: {e}");
            return ExitCode::FAILURE;
        }
    };
    let allowances = match allowances::Allowances::load(&root) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("xtask lint: {e}");
            return ExitCode::FAILURE;
        }
    };
    let ctx = Ctx { root, allowances };

    let mut violations = Vec::new();
    let mut gate = |name: &str, result: Result<Vec<Violation>, String>| match result {
        Ok(v) => violations.extend(v),
        Err(e) => violations.push(Violation::new("gate-error", name.to_owned(), e)),
    };

    gate("profile-audit", profile::check(&ctx));
    gate("dep-direction", deps::check(&ctx));
    gate("file-length", size::check(&ctx));
    gate("game-name", gamenames::check(&ctx));
    gate("doc-link", doclinks::check(&ctx));

    report(&mut violations)
}

fn report(violations: &mut [Violation]) -> ExitCode {
    if violations.is_empty() {
        println!("xtask lint: all gates pass");
        return ExitCode::SUCCESS;
    }
    violations.sort_by(|a, b| a.rule.cmp(b.rule).then_with(|| a.at.cmp(&b.at)));
    for v in violations.iter() {
        println!("error[{}]: {}\n  --> {}", v.rule, v.detail, v.at);
    }
    println!("\nxtask lint: {} violation(s)", violations.len());
    ExitCode::FAILURE
}

/// The workspace root, found by walking up from this crate's manifest directory.
fn workspace_root() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.parent().ok_or("xtask has no parent directory")?;
    if root.join("Cargo.toml").is_file() {
        Ok(root.to_path_buf())
    } else {
        Err(format!("no workspace manifest above {}", manifest.display()))
    }
}

/// Every `*.rs` file under `crates/` and `xtask/`, in a stable order.
pub fn rust_sources(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for top in ["crates", "xtask"] {
        collect(&root.join(top), "rs", &mut out)?;
    }
    out.sort();
    Ok(out)
}

/// Every file with the given extension under `dir`, recursively.
pub fn collect(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect(&path, ext, out)?;
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
    Ok(())
}

/// A path rendered relative to the workspace root, with forward slashes, for stable
/// messages and stable allowance keys across platforms.
pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}
