//! Contract tests for resolution types and their tags.

use super::*;
use crate::gap::ResolutionGap;

#[test]
fn invariant_r1_with_partially_merged() {
    assert!(Outcome::SingleWinner.permits_shadowed_participants());
    assert!(Outcome::Deleted.permits_shadowed_participants());
    assert!(!Outcome::PartiallyMerged.permits_shadowed_participants());
    assert!(!Outcome::Coexist.permits_shadowed_participants());
    assert!(!Outcome::MergedInternally.permits_shadowed_participants());
    assert!(!Outcome::Unknown.permits_shadowed_participants());
}

#[test]
fn defaults_are_the_no_claim_variants() {
    assert_eq!(Outcome::default(), Outcome::Unknown);
    assert_eq!(Mechanism::default(), Mechanism::Undetermined);
    assert_eq!(Confidence::default(), Confidence::Fallback);
    assert_eq!(Agreement::default(), Agreement::Differs);
}

#[test]
fn mechanism_and_merge_tags() {
    let tags: Vec<&str> = [
        Mechanism::ReplacePath,
        Mechanism::SamePath,
        Mechanism::ReplaceTier,
        Mechanism::FilenameOrder,
        Mechanism::LoadOrder,
        Mechanism::DirectoryMerge,
        Mechanism::Undetermined,
    ]
    .iter()
    .map(|m| m.as_tag())
    .collect();
    assert_eq!(
        tags,
        [
            "replace_path",
            "same_path",
            "replace_tier",
            "filename_order",
            "load_order",
            "directory_merge",
            "undetermined"
        ]
    );
    assert_eq!(crate::MergeMode::PathOnly.as_tag(), "path_only");
    assert_eq!(Outcome::PartiallyMerged.as_tag(), "partially_merged");
}

#[test]
fn resolution_gap_tags_match_contract() {
    use ResolutionGap::*;
    let got: Vec<&str> = [
        OrderUnknown,
        ActivationUnknown,
        MergeModeUnknown,
        SamePathUnknown,
        FilenameKeyForm,
        VanillaFilenameAxis,
        ReplacePathScope,
        InFileOrder,
        ChildDigestsUnavailable,
        ParseIncomplete,
        IdentityCollision,
        FileKeyTie,
    ]
    .iter()
    .map(|g| g.as_tag())
    .collect();
    assert_eq!(
        got,
        [
            "order_unknown",
            "activation_unknown",
            "merge_mode_unknown",
            "same_path_unknown",
            "filename_key_form",
            "vanilla_filename_axis",
            "replace_path_scope",
            "in_file_order",
            "child_digests_unavailable",
            "parse_incomplete",
            "identity_collision",
            "file_key_tie",
        ]
    );
}

#[test]
fn shadow_cause_carries_the_contender_that_won() {
    use crate::ids::{PathId, SourceId};
    let c = crate::Contender { source: SourceId(2), path: PathId(5), slot: Some(1) };
    assert_eq!(ShadowCause::KeyOverride(c), ShadowCause::KeyOverride(c));
    assert_ne!(ShadowCause::KeyOverride(c), ShadowCause::ReplaceTier(c));
    assert_ne!(ShadowCause::SamePath(SourceId(1)), ShadowCause::ReplacePath(SourceId(1)));
}

#[test]
fn agreement_is_the_deliberate_exhaustive_exception() {
    // Exhaustive without a wildcard arm: this match is the assertion. If `Agreement`
    // ever gains `#[non_exhaustive]` or a fourth variant, this stops compiling —
    // which is exactly the property ADR-0002 § 3 asks for.
    let tag = match Agreement::LosersMatchWinner {
        Agreement::AllIdentical => "all_identical",
        Agreement::LosersMatchWinner => "losers_match_winner",
        Agreement::Differs => "differs",
    };
    assert_eq!(tag, Agreement::LosersMatchWinner.as_tag());
}
