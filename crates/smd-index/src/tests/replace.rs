use smd_core::{Activation, Mechanism, Outcome, ResolutionGap, ShadowCause, SourceId};

use super::{contender, pid};
use crate::{PlaysetBuilder, ReplacePathScope};

const F: &str = "common/x/a.txt";

#[test]
fn replace_path_vanilla_stated() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.file(SourceId::VANILLA, F, "v").file(a, "other.txt", "o").replace_path(a, "common/x");
    let ix = b.build();
    let p = pid(&ix, F);
    let v = ix.path_verdict(p);
    assert_eq!(v.resolution.outcome, Outcome::Deleted);
    assert_eq!(v.resolution.mechanism, Mechanism::ReplacePath);
    assert!(v.resolution.gaps.is_empty());
    assert_eq!(
        v.shadows.as_slice(),
        &[(contender(SourceId::VANILLA, p), ShadowCause::ReplacePath(a))]
    );
    assert!(v.live.is_empty());
    assert_eq!(ix.derived().shadowed().len(), 1);
}

fn earlier_mod(scope: ReplacePathScope, declarer_rank: &[u32]) -> (crate::ScanIndex, SourceId) {
    let mut b = PlaysetBuilder::new();
    let early = b.add_mod("Early", &[2, 0], Activation::Active);
    let d = b.add_mod("Decl", declarer_rank, Activation::Active);
    b.file(early, F, "e").replace_path(d, "common/x").replace_scope(scope);
    (b.build(), early)
}

#[test]
fn replace_path_earlier_mod_scope_enumerated() {
    // Unknown scope, declarer later: the readings disagree.
    let (ix, _) = earlier_mod(ReplacePathScope::Unknown, &[2, 1]);
    let r = ix.resolve_path(pid(&ix, F));
    assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None));
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::ReplacePathScope]);
    // Scope settled to vanilla_and_earlier: a stated drop.
    let (ix, early) = earlier_mod(ReplacePathScope::VanillaAndEarlier, &[2, 1]);
    let p = pid(&ix, F);
    let v = ix.path_verdict(p);
    assert!(v.resolution.gaps.is_empty());
    assert_eq!(v.shadows[0], (contender(early, p), ShadowCause::ReplacePath(SourceId(2))));
    // Both readings agree (declarer is earlier): nothing dropped, no gap recorded.
    let (ix, early) = earlier_mod(ReplacePathScope::Unknown, &[1, 9]);
    let p = pid(&ix, F);
    let r = ix.resolve_path(p);
    assert!(r.gaps.is_empty());
    assert_eq!(r.winner, Some(contender(early, p)));
    // Vanilla-only reading: kept.
    let (ix, _) = earlier_mod(ReplacePathScope::VanillaOnly, &[2, 1]);
    assert!(ix.resolve_path(pid(&ix, F)).gaps.is_empty());
}

#[test]
fn replace_path_unknown_declarer_not_stated() {
    let mut b = PlaysetBuilder::new();
    let d = b.add_mod("Decl", &[2, 0], Activation::Unknown);
    b.file(SourceId::VANILLA, F, "v").file(d, "z.txt", "z").replace_path(d, "common/x");
    let ix = b.build();
    let p = pid(&ix, F);
    let v = ix.path_verdict(p);
    assert!(v.shadows.is_empty(), "nothing stated dropped");
    assert!(ix.derived().shadowed().is_empty());
    assert_eq!(v.resolution.outcome, Outcome::Unknown);
    assert_eq!(v.resolution.gaps.as_slice(), &[ResolutionGap::ActivationUnknown]);
    let slot = ix.overlay().slot_of(p, SourceId::VANILLA).unwrap();
    assert!(ix.derived().is_possibly(slot) && !ix.derived().is_surviving(slot));
}

#[test]
fn replace_path_inactive_declarer_is_ignored_and_prefix_is_a_boundary() {
    let mut b = PlaysetBuilder::new();
    let d = b.add_mod("Decl", &[2, 0], Activation::Inactive);
    b.file(SourceId::VANILLA, F, "v").file(SourceId::VANILLA, "common/xylophone/a.txt", "v");
    b.replace_path(d, "common/x");
    let ix = b.build();
    assert_eq!(ix.resolve_path(pid(&ix, F)).outcome, Outcome::SingleWinner);
    assert!(!crate::covers("common/x", "common/xylophone/a.txt"));
}

#[test]
fn replace_path_dlc_target_is_a_scope_gap_under_every_reading() {
    for scope in [
        ReplacePathScope::Unknown,
        ReplacePathScope::VanillaOnly,
        ReplacePathScope::VanillaAndEarlier,
    ] {
        let mut b = PlaysetBuilder::new();
        let dlc = b.add_dlc("Dlc", &[1, 0], Activation::Active);
        let d = b.add_mod("Decl", &[2, 1], Activation::Active);
        b.file(dlc, F, "d").replace_path(d, "common/x").replace_scope(scope);
        let ix = b.build();
        let r = ix.resolve_path(pid(&ix, F));
        assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None), "{scope:?}");
        assert!(r.gaps.contains(&ResolutionGap::ReplacePathScope), "{scope:?}");
    }
}

#[test]
fn replace_prefix_is_interned_and_resolves_to_text() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.file(a, "other.txt", "o").replace_path(a, "Common\\X/");
    let ix = b.build();
    let p = ix.path_id("common/x").expect("prefix interned");
    let f = smd_core::Finding {
        subject: smd_core::Subject::Directory { declarer: a, prefix: p },
        ..smd_core::Finding::unscored()
    };
    assert_eq!(&*ix.resolve_finding(&f).subject_text, "mod:A: common/x");
    let stray = smd_core::Subject::Directory { declarer: a, prefix: smd_core::PathId(9999) };
    let f = smd_core::Finding { subject: stray, ..smd_core::Finding::unscored() };
    assert!(ix.resolve_finding(&f).subject_text.contains(smd_core::Diagnostic::UNHANDLED));
}
