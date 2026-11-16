//! Contract tests for findings and their total order.

use super::*;
use crate::evidence::EvidenceValue;
use crate::files::Span;
use crate::ids::DomainId;
use crate::sortkey::{FindingSortKey, sort_findings};

fn site(source: u32, path: u32, offset: u32) -> Site {
    Site { source: SourceId(source), path: PathId(path), span: Span { offset, len: 4, line: 1 } }
}

fn base() -> Finding {
    Finding::detected(
        RuleId("core.symbol_collision"),
        Subject::Symbol(SymbolKey { domain: DomainId(1), symbol: SymbolId(1) }),
        vec![Participant { site: site(1, 1, 0), role: Role::Winner }],
        EvidenceBag::new(),
    )
}

#[test]
fn unscored_leaves_score_and_severity_untouched() {
    let u = Finding::unscored();
    assert_eq!(u.score, 0);
    assert_eq!(u.severity, Severity::Info);

    // The functional-update idiom the whole struct is shaped around: every detection
    // field is overwritten, and score/severity survive from `unscored()`.
    let f = Finding {
        rule: RuleId("core.file_overwrite"),
        subject: Subject::Path(PathId(42)),
        participants: vec![Participant { site: site(3, 42, 0), role: Role::Shadowed }],
        facts: EvidenceBag::from_slice(&[("n", EvidenceValue::Int(2))]),
        ..Finding::unscored()
    };
    assert_eq!(f.score, 0);
    assert_eq!(f.severity, Severity::Info);
    assert!(f.is_unscored());
    assert_eq!(f.rule, RuleId("core.file_overwrite"));

    // …and the convenience constructor agrees with it exactly.
    let g = Finding::detected(f.rule, f.subject.clone(), f.participants.clone(), f.facts.clone());
    assert_eq!(g, f);
}

#[test]
fn policy_writing_score_does_not_disturb_detection_fields() {
    let detected = base();
    let mut scored = detected.clone();
    scored.score = 74;
    scored.severity = Severity::High;

    assert!(!scored.is_unscored());
    assert_eq!(scored.rule, detected.rule);
    assert_eq!(scored.subject, detected.subject);
    assert_eq!(scored.participants, detected.participants);
    assert_eq!(scored.facts, detected.facts);
}

/// Key 1: severity descending, key 2: score descending.
#[test]
fn findings_sort_by_severity_then_score_descending() {
    let mut fs = Vec::new();
    for (sev, score) in
        [(Severity::Low, 10), (Severity::Critical, 1), (Severity::Low, 90), (Severity::High, 50)]
    {
        let mut f = base();
        f.severity = sev;
        f.score = score;
        f.subject = Subject::Path(PathId(score as u32));
        fs.push(f);
    }
    sort_findings(&mut fs);
    let got: Vec<(Severity, i32)> = fs.iter().map(|f| (f.severity, f.score)).collect();
    assert_eq!(
        got,
        vec![
            (Severity::Critical, 1),
            (Severity::High, 50),
            (Severity::Low, 90),
            (Severity::Low, 10)
        ]
    );
}

/// Keys 5 and 6 are what make the order total. Two findings that agree on keys 1–4 must
/// still order deterministically, and no two distinct findings may tie.
#[test]
fn the_sort_key_is_total_over_distinct_findings() {
    let mut fs = Vec::new();

    // Differ only in the participant vector (key 5).
    for source in [1u32, 2, 3] {
        let mut f = base();
        f.participants = vec![Participant { site: site(source, 1, 0), role: Role::Winner }];
        fs.push(f);
    }
    // Differ only in span offset within the same source and path (key 5, third field).
    for offset in [7u32, 9] {
        let mut f = base();
        f.participants = vec![Participant { site: site(1, 1, offset), role: Role::Winner }];
        fs.push(f);
    }
    // Differ only in evidence (key 6).
    for n in [1i64, 2, 3] {
        let mut f = base();
        f.facts = EvidenceBag::from_slice(&[("n", EvidenceValue::Int(n))]);
        fs.push(f);
    }
    // Differ only in subject (key 4), including the three widened variants.
    for subject in [
        Subject::LocKey { lang: LangId(0), key: SymbolId(1) },
        Subject::LocKey { lang: LangId(1), key: SymbolId(1) },
        Subject::Event { namespace: SymbolId(1), ordinal: EventOrdinal::Num(100) },
        Subject::Event { namespace: SymbolId(1), ordinal: EventOrdinal::Name(SymbolId(2)) },
        Subject::Directory { declarer: SourceId(4), prefix: PathId(5) },
        Subject::Mod(SourceId(9)),
        Subject::ModPair(SourceId(1), SourceId(2)),
    ] {
        let mut f = base();
        f.subject = subject;
        fs.push(f);
    }

    let keys: Vec<FindingSortKey> = fs.iter().map(FindingSortKey::of).collect();
    for (i, a) in keys.iter().enumerate() {
        for (j, b) in keys.iter().enumerate() {
            if i == j {
                continue;
            }
            assert_ne!(
                a.cmp(b),
                std::cmp::Ordering::Equal,
                "distinct findings {i} and {j} tie: {:?} vs {:?}",
                fs[i],
                fs[j]
            );
        }
    }
}

/// The order must not depend on the order a rule pushed evidence in, nor on the input
/// order of the vector being sorted.
#[test]
fn sorting_is_stable_under_input_permutation_and_evidence_order() {
    let mut a = base();
    a.facts = EvidenceBag::from_slice(&[
        ("winner", EvidenceValue::Src(SourceId(2))),
        ("n", EvidenceValue::Int(3)),
    ]);
    let mut b = base();
    b.facts = EvidenceBag::from_slice(&[
        ("n", EvidenceValue::Int(3)),
        ("winner", EvidenceValue::Src(SourceId(2))),
    ]);
    assert_eq!(FindingSortKey::of(&a), FindingSortKey::of(&b));

    let mut c = base();
    c.subject = Subject::Path(PathId(1));
    let mut forward = vec![a.clone(), c.clone()];
    let mut backward = vec![c, a];
    sort_findings(&mut forward);
    sort_findings(&mut backward);
    assert_eq!(forward, backward);
}

#[test]
fn subject_order_is_by_variant_then_field() {
    let mut subs = vec![
        Subject::ModPair(SourceId(0), SourceId(1)),
        Subject::Mod(SourceId(0)),
        Subject::Path(PathId(5)),
        Subject::Path(PathId(1)),
        Subject::Symbol(SymbolKey { domain: DomainId(0), symbol: SymbolId(0) }),
    ];
    subs.sort();
    assert_eq!(
        subs,
        vec![
            Subject::Path(PathId(1)),
            Subject::Path(PathId(5)),
            Subject::Symbol(SymbolKey { domain: DomainId(0), symbol: SymbolId(0) }),
            Subject::Mod(SourceId(0)),
            Subject::ModPair(SourceId(0), SourceId(1)),
        ]
    );
}

#[test]
fn severity_declaration_order_is_severity_order() {
    assert!(Severity::Info < Severity::Low);
    assert!(Severity::Low < Severity::Medium);
    assert!(Severity::Medium < Severity::High);
    assert!(Severity::High < Severity::Critical);
    assert_eq!(Severity::default(), Severity::Info);
}
