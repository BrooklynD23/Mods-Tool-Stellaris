//! Enumeration bounds (doc 05, "Unbounded trees").

/// Per-source bounds on a walk. Every bound that trips is a diagnostic plus an
/// incompleteness, never a silent drop.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct WalkLimits {
    /// Directory levels below the root. A directory at level `depth + 1` is not entered.
    pub depth: usize,
    /// Directory entries (files, directories, links, other) enumerated in total.
    pub entries: usize,
    /// Length of a root-relative path in bytes.
    pub rel_path: usize,
    /// Largest file that is read and hashed, in bytes.
    pub file_cap: u64,
}

impl Default for WalkLimits {
    fn default() -> Self {
        WalkLimits { depth: 64, entries: 250_000, rel_path: 1024, file_cap: 32 * 1024 * 1024 }
    }
}
