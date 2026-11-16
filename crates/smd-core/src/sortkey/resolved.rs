//! The output order, over [`ResolvedFinding`] text.

use std::cmp::{Ordering, Reverse};

use crate::finding::Certainty;
use crate::gap::ResolutionGap;
use crate::hash::{ContentHash, Hasher128};
use crate::resolved::{ResolvedEvidence, ResolvedFinding, SubjectPart};

use super::subject_ord::variant;

type ParticipantKey = (Box<str>, Box<str>, u32, u32, u8);

/// The comparable projection of a [`ResolvedFinding`], in the documented key order.
///
/// Owned, so a sort can cache it. Contains no interned id's numeric value.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedSortKey {
    severity_desc: Reverse<u8>,
    score_desc: Reverse<i32>,
    rule: &'static str,
    subject_variant: u8,
    subject_parts: Vec<SubjectPart>,
    subject_text: Box<str>,
    certainty: Certainty,
    participants: Vec<ParticipantKey>,
    evidence: ContentHash,
}

impl ResolvedSortKey {
    /// Project a resolved finding onto its total sort key.
    #[must_use]
    pub fn of(f: &ResolvedFinding) -> ResolvedSortKey {
        ResolvedSortKey {
            severity_desc: Reverse(f.severity as u8),
            score_desc: Reverse(f.score),
            rule: f.rule.as_str(),
            subject_variant: variant(&f.subject),
            subject_parts: f.subject_parts.clone(),
            subject_text: f.subject_text.clone(),
            certainty: f.certainty,
            participants: f
                .participants
                .iter()
                .map(|p| {
                    (p.mod_key.0.clone(), p.path.clone(), p.span.offset, p.span.len, p.role as u8)
                })
                .collect(),
            evidence: digest(&f.facts, &f.gaps),
        }
    }
}

impl Ord for ResolvedSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.severity_desc
            .cmp(&other.severity_desc)
            .then_with(|| self.score_desc.cmp(&other.score_desc))
            .then_with(|| self.rule.as_bytes().cmp(other.rule.as_bytes()))
            .then_with(|| self.subject_variant.cmp(&other.subject_variant))
            .then_with(|| self.subject_parts.cmp(&other.subject_parts))
            .then_with(|| self.subject_text.as_bytes().cmp(other.subject_text.as_bytes()))
            .then_with(|| self.certainty.cmp(&other.certainty))
            .then_with(|| self.participants.cmp(&other.participants))
            .then_with(|| self.evidence.cmp(&other.evidence))
    }
}

impl PartialOrd for ResolvedSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Each value tagged by variant and length-prefixed, as the bytes that are hashed and
/// that break ties between two values under one key.
fn encode(v: &ResolvedEvidence) -> (u8, Vec<u8>) {
    match v {
        ResolvedEvidence::Int(i) => (0, i.to_le_bytes().to_vec()),
        ResolvedEvidence::Bool(b) => (1, vec![u8::from(*b)]),
        ResolvedEvidence::Tag(t) => (2, t.as_bytes().to_vec()),
        ResolvedEvidence::Text(t) => (3, t.as_bytes().to_vec()),
    }
}

/// Sort key 7: a digest over the resolved evidence **sorted by key**, each value tagged by
/// variant and length-prefixed, so the result depends on neither the order a rule pushed
/// entries in nor on any interner.
#[must_use]
pub fn canonical_resolved_evidence_hash(facts: &[(&'static str, ResolvedEvidence)]) -> ContentHash {
    digest(facts, &[])
}

/// Key 7 as `of` uses it: the evidence digest with the finding's gaps (sorted, by tag)
/// folded in, so two findings differing only in why they could not name a winner are
/// still ordered.
fn digest(facts: &[(&'static str, ResolvedEvidence)], gaps: &[ResolutionGap]) -> ContentHash {
    let mut entries: Vec<(&str, u8, Vec<u8>)> = facts
        .iter()
        .map(|(k, v)| {
            let (tag, bytes) = encode(v);
            (*k, tag, bytes)
        })
        .collect();
    entries.sort_unstable();

    let mut h = Hasher128::new();
    h.write_u64(entries.len() as u64);
    for (key, tag, bytes) in &entries {
        h.write_str(key);
        h.write_u8(*tag);
        h.write_framed(bytes);
    }
    let mut tags: Vec<&str> = gaps.iter().map(|g| g.as_tag()).collect();
    tags.sort_unstable();
    h.write_u64(tags.len() as u64);
    for t in tags {
        h.write_str(t);
    }
    h.finish()
}

/// Sort resolved findings into the documented total order.
pub fn sort_resolved_findings(findings: &mut [ResolvedFinding]) {
    findings.sort_by_cached_key(ResolvedSortKey::of);
}
