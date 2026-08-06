//! Contract tests for the fact layer: the discriminant snapshot, and both halves of
//! Invariant C1.

use super::*;

fn sample_payloads() -> Vec<FactPayload<Box<str>>> {
    vec![
        FactPayload::Definition {
            domain: DomainId(2),
            symbol: "fanatic_purifiers".into(),
            body_hash: [1; 16],
        },
        FactPayload::LocKey {
            lang: LangId(0),
            key: "TRAIT_INTELLIGENT".into(),
            version_ordinal: Some(0),
            value_hash: [2; 16],
        },
        FactPayload::Reference { domain: DomainId(3), symbol: "root".into() },
        FactPayload::Namespace { symbol: "my_mod".into() },
        FactPayload::EventId {
            namespace: "my_mod".into(),
            ordinal: EventOrdinal::Name("setup".into()),
        },
        FactPayload::DefineKey {
            group: "NGameplay".into(),
            key: "MAX_FLEETS".into(),
            value_hash: [3; 16],
        },
    ]
}

/// The append-only snapshot. Editing these numbers is a `CONTRACT_VERSION` bump, never a
/// fix — rkyv and postcard encode a variant by position, so a shifted discriminant
/// decodes every cached fact as the wrong kind.
#[test]
fn fact_payload_discriminants_are_pinned() {
    let expected: [u8; 6] = [0, 1, 2, 3, 4, 5];
    let actual: Vec<u8> = sample_payloads().iter().map(FactPayload::discriminant).collect();
    assert_eq!(actual, expected);
    assert_eq!(crate::CONTRACT_VERSION, 1, "discriminant snapshot is pinned to contract v1");
}

#[test]
fn merge_mode_tags_are_snake_case_and_distinct() {
    let all = [
        MergeMode::FileReplace,
        MergeMode::MergeByKey,
        MergeMode::MergeByKeyAdditiveBody,
        MergeMode::Additive,
        MergeMode::Single,
        MergeMode::Unknown,
    ];
    let mut tags: Vec<&str> = all.iter().map(|m| m.as_tag()).collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), all.len());
    assert_eq!(MergeMode::default(), MergeMode::Unknown);
}

/// Invariant C1, positive half: `RawFact` is the *only* fact form the cache stores, so it
/// must survive a serde round-trip byte-for-byte.
#[cfg(feature = "serde")]
#[test]
fn raw_fact_round_trips_through_serde() {
    for payload in sample_payloads() {
        let fact = RawFact { span: Span { offset: 40, len: 12, line: 3 }, payload };
        let json = serde_json::to_string(&fact).expect("RawFact must serialise");
        let back: RawFact = serde_json::from_str(&json).expect("RawFact must deserialise");
        assert_eq!(back, fact);
    }
}

/// Invariant C1, negative half: `Fact` carries interned ids, so it must not implement
/// `Serialize` at all — not "must not be serialised by convention".
#[cfg(feature = "serde")]
#[test]
fn fact_does_not_implement_serialize() {
    assert!(!implements!(Fact: serde::Serialize));
    assert!(!implements!(FactPayload<SymbolId>: serde::Serialize));
    assert!(!implements!(crate::Site: serde::Serialize));
    assert!(!implements!(crate::SourceId: serde::Serialize));
    assert!(!implements!(crate::PathId: serde::Serialize));
    assert!(!implements!(SymbolId: serde::Serialize));
    // …while the cacheable form does, so the probe is measuring something.
    assert!(implements!(RawFact: serde::Serialize));
    assert!(implements!(FactPayload<Box<str>>: serde::Serialize));
}
