//! `smd-policy` — Valuation as data: weights, modifiers, bands, certainty caps,
//! explanations, and suppressions.
//!
//! **Owns:** the only code that writes `ResolvedFinding::score` and `severity`; the
//! explanation catalogue lookup; the user suppression file.
//!
//! **Must never:** detect anything; take `&ScanIndex` (findings arrive already resolved
//! to text by `smd-index`, so this crate depends on `smd-core` and, for its glob, on
//! `smd-profiles`).
//!
//! # Pipeline (doc 09)
//!
//! base weight → phase-1 modifiers → phase-2 (aggregate) modifiers → clamp → band →
//! certainty caps → explanation → suppression → sort. The aggregate population is every
//! finding the scan produced, before suppression, minus `Conditional` activation.
//!
//! The shipped data is `policy/weights.toml` and `policy/explain/en.toml`, embedded;
//! [`Policy::from_dir`] reads the same layout from disk. Every number is UNCALIBRATED
//! until M5's labelled ground truth exists.

#![forbid(unsafe_code)]

mod appdir;
mod explain;
mod load;
mod score;
mod suppress;
#[cfg(test)]
mod tests;

use std::path::Path;

use smd_core::ResolvedFinding;

pub(crate) use appdir::{APP_DIR_WINDOWS, APP_DIR_XDG};
pub use explain::Explanation;
pub use suppress::{
    SUPPRESSIONS_VERSION, Suppression, Suppressions, default_path as default_suppressions_path,
    is_finding_id,
};

const SHIPPED_WEIGHTS: &str = include_str!("../../../policy/weights.toml");
const SHIPPED_EXPLAIN_EN: &str = include_str!("../../../policy/explain/en.toml");

/// Why policy data could not be used.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PolicyError {
    /// Not valid TOML for the schema.
    Syntax(String),
    /// Valid TOML, invalid policy (a missing cap row, bands out of order, ...).
    Invalid(String),
    /// A file could not be read or written.
    Io(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::Syntax(e) => write!(f, "policy file does not parse: {e}"),
            PolicyError::Invalid(e) => write!(f, "policy is invalid: {e}"),
            PolicyError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for PolicyError {}

/// Loaded policy: weights and the English catalogue.
#[derive(Clone, Debug)]
pub struct Policy {
    weights: load::Weights,
    catalogue: explain::Catalogue,
}

/// What policy added to one finding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Annotation {
    /// Stable finding id: rule and subject, hashed. What `smd suppress` takes.
    pub id: String,
    /// `None` when the catalogue has no entry for the rule.
    pub explanation: Option<Explanation>,
    /// Hidden by a user suppression. Still reported, never dropped.
    pub suppressed: bool,
}

/// Scored findings, in output order, with one annotation each (same index).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Scored {
    /// Score and severity written, sorted by `smd-core`'s text order.
    pub findings: Vec<ResolvedFinding>,
    /// `annotations[i]` belongs to `findings[i]`.
    pub annotations: Vec<Annotation>,
    /// Rule ids that produced findings but have no `[base]` weight (they score 0).
    pub unweighted: Vec<String>,
}

/// The finding id: the first 8 bytes of blake3 over `rule \0 subject`, as hex. Stable
/// across runs and machines; independent of which mods take part.
#[must_use]
pub fn finding_id(f: &ResolvedFinding) -> String {
    let mut h = blake3::Hasher::new();
    h.update(f.rule.as_str().as_bytes());
    h.update(&[0]);
    h.update(f.subject_text.as_bytes());
    h.finalize().as_bytes()[..8].iter().map(|b| format!("{b:02x}")).collect()
}

impl Policy {
    /// The policy this build embeds.
    ///
    /// # Errors
    /// [`PolicyError`] when the embedded data is invalid; a test pins that it is not.
    pub fn shipped() -> Result<Policy, PolicyError> {
        Policy::from_texts(SHIPPED_WEIGHTS, SHIPPED_EXPLAIN_EN)
    }

    /// Read `<dir>/weights.toml` and `<dir>/explain/en.toml`.
    ///
    /// # Errors
    /// [`PolicyError`] when either file is unreadable or invalid.
    pub fn from_dir(dir: &Path) -> Result<Policy, PolicyError> {
        let read = |p: &Path| {
            std::fs::read_to_string(p).map_err(|e| PolicyError::Io(format!("{}: {e}", p.display())))
        };
        let weights = read(&dir.join("weights.toml"))?;
        let explain = read(&dir.join("explain").join("en.toml"))?;
        Policy::from_texts(&weights, &explain)
    }

    /// From the two files' text.
    ///
    /// # Errors
    /// [`PolicyError`] when either is invalid.
    pub fn from_texts(weights: &str, explain_en: &str) -> Result<Policy, PolicyError> {
        Ok(Policy { weights: load::weights(weights)?, catalogue: explain::catalogue(explain_en)? })
    }

    /// Score, band, cap, explain, apply `suppressions`, and sort.
    #[must_use]
    pub fn score(&self, findings: Vec<ResolvedFinding>, suppressions: &Suppressions) -> Scored {
        let counts = score::population(&findings);
        let mut unweighted: Vec<String> = findings
            .iter()
            .map(|f| f.rule.as_str())
            .filter(|r| !self.weights.base.contains_key(*r))
            .map(str::to_owned)
            .collect();
        unweighted.sort_unstable();
        unweighted.dedup();
        let mut pairs: Vec<(ResolvedFinding, Annotation)> = findings
            .into_iter()
            .map(|f| {
                let s = score::raw_score(&self.weights, &f, &counts);
                let severity = score::severity(&self.weights, &f, s);
                let f = ResolvedFinding { score: s, severity, ..f };
                let id = finding_id(&f);
                let suppressed = suppressions.contains(&id);
                let a = Annotation { explanation: self.catalogue.explain(&f), suppressed, id };
                (f, a)
            })
            .collect();
        pairs.sort_by_cached_key(|(f, _)| smd_core::ResolvedSortKey::of(f));
        let (findings, annotations) = pairs.into_iter().unzip();
        Scored { findings, annotations, unweighted }
    }
}
