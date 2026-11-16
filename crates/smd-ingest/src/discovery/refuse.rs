//! Resolving a descriptor's `path=` / `archive=` once, and refusing the dangerous ones.
//!
//! A descriptor cannot turn "scan this mod" into "scan my disk" (doc 05, "Path traversal
//! via descriptors"): a result that is a filesystem root, the home directory, or an
//! ancestor of the game install or of another root is refused.

use std::path::{Component, Path, PathBuf};

/// The roots a descriptor path must not swallow.
#[derive(Clone, Debug, Default)]
pub(super) struct Guard {
    /// Canonical forms of the home directory and every other root, as far as known.
    protected: Vec<PathBuf>,
}

/// Collapse `.` and `..` without touching the filesystem.
pub(super) fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// The canonical form when the path exists, else its lexical form.
pub(super) fn canonical_or_lexical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| lexical(p))
}

impl Guard {
    /// Protect `paths`, each resolved once.
    pub(super) fn new(paths: impl IntoIterator<Item = PathBuf>) -> Guard {
        let mut protected: Vec<PathBuf> =
            paths.into_iter().map(|p| canonical_or_lexical(&p)).collect();
        protected.sort();
        protected.dedup();
        Guard { protected }
    }

    /// Resolve a descriptor path against `base` and decide whether it may be used.
    ///
    /// # Errors
    /// The reason it was refused.
    pub(super) fn resolve(&self, base: &Path, declared: &Path) -> Result<PathBuf, String> {
        let resolved = canonical_or_lexical(&base.join(declared));
        if resolved.parent().is_none() {
            return Err(format!("{} is a filesystem root", resolved.display()));
        }
        if let Some(hit) = self.protected.iter().find(|p| p.starts_with(&resolved)) {
            return Err(format!(
                "{} is, or is an ancestor of, {}",
                resolved.display(),
                hit.display()
            ));
        }
        Ok(resolved)
    }
}
