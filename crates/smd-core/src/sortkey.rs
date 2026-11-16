//! Sort keys.
//!
//! **Output order** is [`sort_resolved_findings`], defined over [`ResolvedFinding`] text
//! (doc 03, "Ordering must be total"; REVIEW-2026-10 H15):
//!
//! | # | Key | Direction |
//! | --- | --- | --- |
//! | 1 | `severity` | descending |
//! | 2 | `score` | descending |
//! | 3 | `rule` | byte-lexicographic |
//! | 4 | `subject` variant discriminant, then each field's resolved text in declaration order (`EventOrdinal`: `Num` before `Name`, `Num` numerically) | ascending |
//! | 5 | `certainty`, four axes best-first | ascending |
//! | 6 | participants as `(mod_key, path, span.offset, span.len, role)` | lexicographic |
//! | 7 | canonical hash over the resolved evidence (and the gaps) | ascending |
//!
//! No interned id's numeric value is an output key. [`FindingSortKey`] / [`sort_findings`]
//! order the *unresolved* [`Finding`] by interned ids and are for in-engine use (stable
//! within one scan) only; they must not decide what a user sees.
//!
//! Nothing here reads a `HashMap`.

use std::cmp::Ordering;

use crate::evidence::{EvidenceBag, EvidenceValue};
use crate::facts::EventOrdinal;
use crate::finding::{Certainty, Finding, Subject};
use crate::hash::{ContentHash, Hasher128};

mod resolved;
#[cfg(test)]
mod resolved_tests;
mod subject_ord;

pub use resolved::{ResolvedSortKey, canonical_resolved_evidence_hash, sort_resolved_findings};

/// The comparable projection of a [`Finding`], in the documented key order.
///
/// Built by value so a sort can key on it directly without borrowing the finding.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FindingSortKey {
    severity_desc: std::cmp::Reverse<u8>,
    score_desc: std::cmp::Reverse<i32>,
    rule: &'static str,
    subject: Subject,
    certainty: Certainty,
    participants: Vec<(u32, u32, u32)>,
    evidence: ContentHash,
}

impl FindingSortKey {
    /// Project a finding onto its total sort key.
    #[must_use]
    pub fn of(f: &Finding) -> FindingSortKey {
        FindingSortKey {
            severity_desc: std::cmp::Reverse(f.severity as u8),
            score_desc: std::cmp::Reverse(f.score),
            rule: f.rule.as_str(),
            subject: f.subject.clone(),
            certainty: f.certainty,
            participants: f.participants.iter().map(|p| p.site.sort_key()).collect(),
            evidence: canonical_evidence_hash(&f.facts),
        }
    }
}

impl Ord for FindingSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.severity_desc
            .cmp(&other.severity_desc)
            .then_with(|| self.score_desc.cmp(&other.score_desc))
            .then_with(|| self.rule.cmp(other.rule))
            .then_with(|| self.subject.cmp(&other.subject))
            .then_with(|| self.certainty.cmp(&other.certainty))
            .then_with(|| self.participants.cmp(&other.participants))
            .then_with(|| self.evidence.cmp(&other.evidence))
    }
}

impl PartialOrd for FindingSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Sort key 6: a digest over the evidence bag **sorted by key**, so that the result does
/// not depend on the order a rule happened to push entries in.
#[must_use]
pub fn canonical_evidence_hash(bag: &EvidenceBag) -> ContentHash {
    let mut entries: Vec<&(&'static str, EvidenceValue)> = bag.iter().collect();
    entries.sort_unstable_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));

    let mut h = Hasher128::new();
    h.write_u64(entries.len() as u64);
    for (key, value) in entries {
        h.write_str(key);
        h.write_u8(value.kind_ordinal());
        match *value {
            EvidenceValue::Int(v) => h.write_u64(v as u64),
            EvidenceValue::Bool(v) => h.write_u8(u8::from(v)),
            EvidenceValue::Tag(v) => h.write_str(v),
            EvidenceValue::Sym(v) => h.write_u64(u64::from(v.0)),
            EvidenceValue::Path(v) => h.write_u64(u64::from(v.0)),
            EvidenceValue::Src(v) => h.write_u64(u64::from(v.index())),
        }
    }
    h.finish()
}

/// Sort a slice of findings into the documented total order.
pub fn sort_findings(findings: &mut [Finding]) {
    findings.sort_by_cached_key(FindingSortKey::of);
}

/// The ordinal an [`EventOrdinal`] contributes to [`Subject`]'s order.
pub(crate) fn event_ordinal_key(o: &EventOrdinal<crate::SymbolId>) -> (u8, u32) {
    match o {
        EventOrdinal::Num(n) => (0, *n),
        EventOrdinal::Name(s) => (1, s.0),
    }
}
