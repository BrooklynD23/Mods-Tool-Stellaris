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
        FactPayload::AdditiveSplit {
            domain: DomainId(4),
            symbol: "on_game_start".into(),
            residual_hash: [4; 16],
            additive_count: 2,
        },
    ]
}

/// The append-only snapshot. Editing these numbers is a `CONTRACT_VERSION` bump, never a
/// fix — rkyv and postcard encode a variant by position, so a shifted discriminant
/// decodes every cached fact as the wrong kind.
#[test]
fn fact_payload_discriminants_are_pinned() {
    // Definition, LocKey, Reference, Namespace, EventId, DefineKey, AdditiveSplit.
    let expected: [u8; 7] = [0, 1, 2, 3, 4, 5, 6];
    let actual: Vec<u8> = sample_payloads().iter().map(FactPayload::discriminant).collect();
    assert_eq!(actual, expected);
    assert_eq!(crate::CONTRACT_VERSION, 1, "discriminant snapshot is pinned to contract v1");
}

#[test]
fn merge_mode_tags_are_snake_case_and_distinct() {
    let all = [
        MergeMode::PathOnly,
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
    assert_eq!(MergeMode::PathOnly.as_tag(), "path_only");
}

fn class() -> ExtractionClass {
    ExtractionClass {
        kind: crate::FileKind::Script,
        parser: "clausewitz",
        extractor: "depth_key",
        level: 0,
        domain: Some("trait".into()),
        additive_children: Box::new(["a".into(), "b".into()]),
    }
}

#[test]
fn extraction_class_key_bytes_are_canonical() {
    let base = class();
    let other_domain = ExtractionClass { domain: Some("civic".into()), ..class() };
    assert_ne!(base.key_bytes(), other_domain.key_bytes());

    let reordered =
        ExtractionClass { additive_children: Box::new(["b".into(), "a".into()]), ..class() };
    assert_eq!(base.key_bytes(), reordered.key_bytes());

    let ab_c =
        ExtractionClass { additive_children: Box::new(["ab".into(), "c".into()]), ..class() };
    let a_bc =
        ExtractionClass { additive_children: Box::new(["a".into(), "bc".into()]), ..class() };
    assert_ne!(ab_c.key_bytes(), a_bc.key_bytes());

    let once = ExtractionClass { additive_children: Box::new(["a".into()]), ..class() };
    let twice =
        ExtractionClass { additive_children: Box::new(["a".into(), "a".into()]), ..class() };
    assert_eq!(once.key_bytes(), twice.key_bytes(), "a set, not a multiset");

    let none = ExtractionClass { domain: None, ..class() };
    let empty = ExtractionClass { domain: Some("".into()), ..class() };
    assert_ne!(none.key_bytes(), empty.key_bytes());
}

/// `FileKind` is a parse-cache key component: its numbers are pinned, never renumbered.
#[test]
fn file_kind_discriminants_are_pinned() {
    use crate::FileKind::*;
    let actual = [Script, Localisation, Asset, Descriptor, Gui, Unknown].map(|k| k as u8);
    assert_eq!(actual, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn same_path_tags() {
    assert_eq!(SamePath::Replace.as_tag(), "replace");
    assert_eq!(SamePath::default(), SamePath::Unknown);
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
