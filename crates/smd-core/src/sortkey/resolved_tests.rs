//! Tests for the text-based output order.

use smallvec::smallvec;

use super::*;
use crate::files::Span;
use crate::finding::{Role, RuleId, Severity, Subject};
use crate::gap::ResolutionGap;
use crate::ids::{ModKey, PathId};
use crate::resolved::{
    ResolvedEvidence, ResolvedFinding, ResolvedOrdinal, ResolvedParticipant, SubjectPart,
};

fn rf(path_id: u32, text: &str) -> ResolvedFinding {
    ResolvedFinding {
        rule: RuleId("core.file_overwrite"),
        subject: Subject::Path(PathId(path_id)),
        subject_text: text.into(),
        subject_parts: Vec::new(),
        participants: Vec::new(),
        facts: smallvec![],
        certainty: Certainty::default(),
        gaps: smallvec![],
        score: 0,
        severity: Severity::Info,
    }
}

fn part(len: u32, role: Role) -> ResolvedParticipant {
    ResolvedParticipant {
        mod_key: ModKey("m".into()),
        mod_name: "M".into(),
        path: "common/a.txt".into(),
        span: Span { offset: 0, len, line: 1 },
        role,
    }
}

#[test]
fn resolved_sort_is_text_based_and_total() {
    // Numeric id order says b (PathId 1) first; text order says a first.
    let mut v = vec![rf(1, "b"), rf(9, "a")];
    sort_resolved_findings(&mut v);
    assert_eq!(&*v[0].subject_text, "a");

    // Differing only in span.len or role: ordered, never Equal.
    let mut a = rf(1, "x");
    a.participants = vec![part(4, Role::Winner)];
    let mut b = rf(1, "x");
    b.participants = vec![part(5, Role::Winner)];
    let mut c = rf(1, "x");
    c.participants = vec![part(4, Role::Shadowed)];
    let keys = [&a, &b, &c].map(ResolvedSortKey::of);
    for i in 0..3 {
        for j in 0..3 {
            if i != j {
                assert_ne!(keys[i].cmp(&keys[j]), Ordering::Equal);
            }
        }
    }

    // Twenty distinct findings, five shuffles, one order.
    let base: Vec<ResolvedFinding> = (0..20u32)
        .map(|i| {
            let mut f = rf(i, &format!("s{}", i % 7));
            f.score = (i % 3) as i32;
            f.participants = vec![part(i, Role::Winner)];
            f
        })
        .collect();
    let mut reference = base.clone();
    sort_resolved_findings(&mut reference);
    for seed in 1..=5usize {
        let mut shuffled = base.clone();
        let n = shuffled.len();
        // Deterministic permutation: i -> (i * step + seed) mod n, step coprime to 20.
        let step = [3, 7, 9, 11, 13][seed - 1];
        shuffled = (0..n).map(|i| base[(i * step + seed) % n].clone()).collect();
        sort_resolved_findings(&mut shuffled);
        assert_eq!(shuffled, reference, "shuffle {seed} changed the order");
    }
}

#[test]
fn evidence_hash_ignores_push_order() {
    let a: [(&'static str, ResolvedEvidence); 2] =
        [("n", ResolvedEvidence::Int(3)), ("w", ResolvedEvidence::Text("x".into()))];
    let b = [a[1].clone(), a[0].clone()];
    assert_eq!(canonical_resolved_evidence_hash(&a), canonical_resolved_evidence_hash(&b));
    let c = [("n", ResolvedEvidence::Int(4)), a[1].clone()];
    assert_ne!(canonical_resolved_evidence_hash(&a), canonical_resolved_evidence_hash(&c));
}

fn event(text: &str, ns: &str, ord: ResolvedOrdinal) -> ResolvedFinding {
    let mut f = rf(0, text);
    f.subject_parts = vec![SubjectPart::Text(ns.into()), SubjectPart::Ordinal(ord)];
    f
}

#[test]
fn event_ordinals_sort_numerically_and_num_before_name() {
    // Byte order of the flat text would put "ns.10" before "ns.9" and "ns.a" before both.
    let mut v = vec![
        event("ns.a", "ns", ResolvedOrdinal::Name("a".into())),
        event("ns.10", "ns", ResolvedOrdinal::Num(10)),
        event("ns.9", "ns", ResolvedOrdinal::Num(9)),
    ];
    sort_resolved_findings(&mut v);
    let texts: Vec<&str> = v.iter().map(|f| &*f.subject_text).collect();
    assert_eq!(texts, ["ns.9", "ns.10", "ns.a"]);
}

#[test]
fn key_priority_follows_the_documented_order() {
    // 1: severity descending, beating a better score and rule.
    let mut hi = rf(1, "z");
    hi.severity = Severity::High;
    let mut lo = rf(1, "a");
    lo.score = 99;
    assert!(ResolvedSortKey::of(&hi) < ResolvedSortKey::of(&lo));

    // 2: score descending.
    let mut s1 = rf(1, "z");
    s1.score = 5;
    assert!(ResolvedSortKey::of(&s1) < ResolvedSortKey::of(&rf(1, "a")));

    // 5: certainty decides before participants.
    let mut worse = rf(1, "x");
    worse.participants = vec![part(0, Role::Winner)];
    let mut better = rf(1, "x");
    better.certainty.activation = crate::finding::ActivationCertainty::Certain;
    better.participants = vec![part(9, Role::Winner)];
    assert!(better.certainty < worse.certainty);
    assert!(ResolvedSortKey::of(&better) < ResolvedSortKey::of(&worse));

    // 6: participants decide before evidence.
    let mut p1 = rf(1, "x");
    p1.participants = vec![part(1, Role::Winner)];
    p1.facts = smallvec![("n", ResolvedEvidence::Int(9))];
    let mut p2 = rf(1, "x");
    p2.participants = vec![part(2, Role::Winner)];
    p2.facts = smallvec![("n", ResolvedEvidence::Int(1))];
    assert!(ResolvedSortKey::of(&p1) < ResolvedSortKey::of(&p2));
}

#[test]
fn findings_differing_only_in_gaps_are_ordered() {
    let mut a = rf(1, "x");
    a.gaps = smallvec![ResolutionGap::OrderUnknown];
    let mut b = rf(1, "x");
    b.gaps = smallvec![ResolutionGap::ActivationUnknown];
    assert_ne!(ResolvedSortKey::of(&a).cmp(&ResolvedSortKey::of(&b)), Ordering::Equal);
}
