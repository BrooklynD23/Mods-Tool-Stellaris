//! `ResolvedFinding` — the policy and presentation boundary.
//!
//! Interned ids stop at the engine's edge (Invariant C1); everything downstream of
//! detection receives text. `smd-index` produces these, being the crate that owns the
//! interners. `smd-policy` needs path text for its `path_glob` modifiers and `smd-report`
//! needs symbol text for `{symbol}`; given a `ResolvedFinding`, neither takes
//! `&ScanIndex`, and the separation the layer map exists to enforce stays real rather
//! than nominal. (REVIEW-2026-08 § G42.)
//!
//! # Why these types are not `Serialize`
//!
//! Doc 03 keeps `subject: Subject` on [`ResolvedFinding`] "for identity, sorting, and
//! suppression", and Invariant C1 forbids interned ids from crossing the API boundary.
//! Both cannot hold of a `Serialize` impl, because [`Subject`] is made of scan-local ids.
//! The boundary type therefore stays in-process: `smd-report` renders it, and `smd-api`
//! projects the fields it exports into its own DTO, which is where `ts-rs` and the
//! no-bare-`u64` rule apply anyway.

use smallvec::SmallVec;

use crate::files::Span;
use crate::finding::{Role, RuleId, Severity, Subject};
use crate::ids::ModKey;

/// A finding with every interned id turned back into text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedFinding {
    /// Which rule emitted it.
    pub rule: RuleId,
    /// Retained for identity, sorting, and suppression. The ids inside are meaningless to
    /// a caller holding a different scan, and are never rendered and never exported.
    pub subject: Subject,
    /// The subject, rendered.
    pub subject_text: Box<str>,
    /// Participants, flattened.
    pub participants: Vec<ResolvedParticipant>,
    /// Evidence, typed and resolved.
    pub facts: SmallVec<[(&'static str, ResolvedEvidence); 4]>,
    /// Written by `smd-policy`.
    pub score: i32,
    /// Written by `smd-policy`.
    pub severity: Severity,
}

/// One participant, with its mod identified by a stable key rather than a scan-local id.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedParticipant {
    /// Stable across runs and machines.
    pub mod_key: ModKey,
    /// Display name.
    pub mod_name: Box<str>,
    /// Normalised, mod-root-relative.
    pub path: Box<str>,
    /// Raw-file byte offsets.
    pub span: Span,
    /// What part it plays.
    pub role: Role,
}

/// An evidence value with its ids resolved. There is no `Sym`/`Path`/`Src` here: those
/// became [`Text`](ResolvedEvidence::Text) at the boundary.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ResolvedEvidence {
    /// A count, a byte size, a score contribution.
    Int(i64),
    /// A predicate.
    Bool(bool),
    /// A closed-vocabulary tag.
    Tag(&'static str),
    /// A resolved symbol, path, or mod name.
    Text(Box<str>),
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;

    /// Nothing carrying a scan-local id is serialisable, at either end of the boundary.
    #[test]
    fn nothing_carrying_an_interned_id_is_serialisable() {
        assert!(!implements!(ResolvedFinding: serde::Serialize));
        assert!(!implements!(crate::Finding: serde::Serialize));
        assert!(!implements!(crate::Participant: serde::Serialize));
        assert!(!implements!(crate::EvidenceValue: serde::Serialize));
        assert!(!implements!(Subject: serde::Serialize));
        // What does leave the engine is text and a stable key.
        assert!(implements!(ModKey: serde::Serialize));
        assert!(implements!(Span: serde::Serialize));
    }
}
