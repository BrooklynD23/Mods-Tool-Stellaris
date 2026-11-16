use smd_core::{
    Activation, Agreement, Mechanism, Outcome, ResolutionGap, SamePath, ShadowCause, SourceId,
    UnparsedReason,
};

use super::{X, contender, pid, three_way};
use crate::{FileShape, PlaysetBuilder};

#[test]
fn h16_path_ids_canonical() {
    let build = |order: &[usize], rev: bool| {
        let (mut b, a, _) = three_way();
        b.file(a, "common/y/a.txt", "1").file(a, "Common/Z/b.txt", "2");
        b.file(SourceId::VANILLA, "events/e.txt", "3").file(SourceId(2), "common/y/a.txt", "4");
        b.build_with(order, rev)
    };
    let base = build(&[0, 1, 2], false);
    let texts: Vec<String> =
        (0..base.path_count()).map(|i| base.path_text(smd_core::PathId(i)).into()).collect();
    let mut sorted = texts.clone();
    sorted.sort();
    assert_eq!(texts, sorted, "ascending NormPath bytes");
    for (order, rev) in [(&[2, 1, 0][..], true), (&[1, 2, 0][..], false), (&[2, 0, 1][..], true)] {
        let other = build(order, rev);
        assert_eq!(other.path_count(), base.path_count());
        for i in 0..base.path_count() {
            let p = smd_core::PathId(i);
            assert_eq!(other.path_text(p), base.path_text(p));
            assert_eq!(other.resolve_path(p), base.resolve_path(p));
            assert_eq!(other.path_verdict(p), base.path_verdict(p));
        }
        assert_eq!(other.derived().shadowed(), base.derived().shadowed());
    }
}

#[test]
fn stage_a_same_path_highest_rank_wins_whole_file() {
    let (b, a, m) = three_way();
    let ix = b.build();
    let p = pid(&ix, X);
    let r = ix.resolve_path(p);
    assert_eq!(r.outcome, Outcome::SingleWinner);
    assert_eq!(r.winner, Some(contender(m, p)));
    assert_eq!(r.mechanism, Mechanism::SamePath);
    assert!(r.gaps.is_empty());
    let shadows = ix.path_verdict(p).shadows;
    assert_eq!(shadows.len(), 2);
    assert!(shadows.contains(&(contender(a, p), ShadowCause::SamePath(m))));
    assert!(shadows.contains(&(contender(SourceId::VANILLA, p), ShadowCause::SamePath(m))));
    assert_eq!(r.discarded.len(), 2);
}

#[test]
fn stage_a_same_path_unknown() {
    let (mut b, ..) = three_way();
    b.same_path(SamePath::Unknown);
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!(r.outcome, Outcome::Unknown);
    assert_eq!(r.winner, None);
    assert_eq!(r.mechanism, Mechanism::Undetermined);
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::SamePathUnknown]);
}

#[test]
fn stage_a_order_unknown() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[], Activation::Active);
    b.file(a, X, "a").file(m, X, "b");
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!(r.outcome, Outcome::Unknown);
    assert_eq!(r.winner, None);
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::OrderUnknown]);
}

#[test]
fn stage_a_activation_unknown_blocks() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let u = b.add_mod("U", &[2, 1], Activation::Unknown);
    b.file(a, X, "a").file(u, X, "u");
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None));
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::ActivationUnknown]);
    assert_eq!(r.mechanism, Mechanism::Undetermined);
}

#[test]
fn stage_a_active_outranking_unknown_is_stated() {
    let mut b = PlaysetBuilder::new();
    let u = b.add_mod("U", &[2, 0], Activation::Unknown);
    let a = b.add_mod("A", &[2, 1], Activation::Active);
    b.file(u, X, "u").file(a, X, "a");
    let ix = b.build();
    let p = pid(&ix, X);
    let r = ix.resolve_path(p);
    assert_eq!(r.outcome, Outcome::SingleWinner);
    assert_eq!(r.winner, Some(contender(a, p)));
    assert!(r.gaps.is_empty());
}

#[test]
fn stage_a_inactive_never_participates() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let off = b.add_mod("Off", &[2, 1], Activation::Inactive);
    b.file(a, X, "a").file(off, X, "off");
    let ix = b.build();
    let p = pid(&ix, X);
    let v = ix.path_verdict(p);
    assert_eq!(v.resolution.winner, Some(contender(a, p)));
    assert!(v.resolution.discarded.is_empty());
    assert_eq!(v.live.as_slice(), &[contender(a, p)]);
    assert!(v.shadows.is_empty());
    let slot = ix.overlay().slot_of(p, off).unwrap();
    assert!(!ix.derived().contends(slot));
}

#[test]
fn stage_a_unreadable_or_changed_contribution_no_winner() {
    for why in
        [UnparsedReason::Unreadable, UnparsedReason::ChangedDuringScan, UnparsedReason::Refused]
    {
        let (mut b, ..) = three_way();
        let c = b.add_mod("C", &[2, 2], Activation::Active);
        b.file_with(c, X, FileShape::unread(why));
        let ix = b.build();
        let r = ix.resolve_path(pid(&ix, X));
        assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None), "{why:?}");
        assert_eq!(r.gaps.as_slice(), &[ResolutionGap::ParseIncomplete]);
        assert_eq!(r.agreement, smd_core::Agreement::Differs);
    }
}

#[test]
fn stage_a_refused_top_contender_is_a_gap_not_a_winner() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, X, "a").file_with(m, X, FileShape::unread(UnparsedReason::Refused));
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None));
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::ParseIncomplete]);
}

#[test]
fn stage_a_oversize_winner_survives_with_differing_agreement() {
    let mut b = PlaysetBuilder::new();
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(SourceId::VANILLA, X, "v");
    // The walker records an oversize file with no blob.
    b.file_with(m, X, FileShape::unread(UnparsedReason::Oversize));
    let ix = b.build();
    let p = pid(&ix, X);
    let r = ix.resolve_path(p);
    assert_eq!(r.outcome, Outcome::SingleWinner);
    assert_eq!(r.winner, Some(contender(m, p)));
    assert!(r.gaps.is_empty());
    assert_eq!(r.agreement, Agreement::Differs);
}

#[test]
fn stage_a_blocking_gaps_do_not_mask_the_others() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.file(a, X, "a").file(m, X, "b");
    b.same_path(SamePath::Unknown);
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    for g in [
        ResolutionGap::SamePathUnknown,
        ResolutionGap::OrderUnknown,
        ResolutionGap::ActivationUnknown,
    ] {
        assert!(r.gaps.contains(&g), "missing {g:?} in {:?}", r.gaps);
    }
}

#[test]
fn stage_a_lone_stated_contributor_is_not_undetermined() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.file(a, X, "a");
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!(r.outcome, Outcome::SingleWinner);
    assert_eq!(r.mechanism, Mechanism::SamePath);
}

#[test]
fn stage_a_identity_collision_gap() {
    let (mut b, a, _) = three_way();
    b.file_with(a, X, FileShape::of("a2").ambiguous());
    let ix = b.build();
    let r = ix.resolve_path(pid(&ix, X));
    assert_eq!((r.outcome, r.winner), (Outcome::Unknown, None));
    assert_eq!(r.gaps.as_slice(), &[ResolutionGap::IdentityCollision]);
}

#[test]
fn agreement_excludes_vanilla() {
    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(SourceId::VANILLA, X, "vanilla").file(a, X, "same").file(m, X, "same");
    let ix = b.build();
    assert_eq!(ix.resolve_path(pid(&ix, X)).agreement, Agreement::LosersMatchWinner);

    let mut b = PlaysetBuilder::new();
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(SourceId::VANILLA, X, "same").file(a, X, "same").file(m, X, "same");
    let ix = b.build();
    assert_eq!(ix.resolve_path(pid(&ix, X)).agreement, Agreement::AllIdentical);

    let (b, ..) = three_way();
    let ix = b.build();
    assert_eq!(ix.resolve_path(pid(&ix, X)).agreement, Agreement::Differs);
}
