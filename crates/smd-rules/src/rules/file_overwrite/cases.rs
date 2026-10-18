//! The fixtures `core.file_overwrite` is defined by.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase};

const X: &str = "common/x/00.txt";

fn two_overhauls(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, X, "a").file(m, X, "b");
}

fn inactive_duplicate(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let off = b.add_mod("Off", &[2, 1], Activation::Inactive);
    b.file(a, X, "a").file(off, X, "b");
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.file(a, X, "a").file(m, X, "b");
    b.directory_scan();
}

fn only_copy_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.file(SourceId::VANILLA, X, "v").replace_path(a, "common/x");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn winner_copy_unreadable(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, X, "a");
    b.file_with(m, X, FileShape::unread(UnparsedReason::Unreadable));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "two overhauls, one file",
        kind: CaseKind::Positive,
        build: two_overhauls,
        expect: Expectation::fires_on_path(X, "B"),
    },
    RuleCase {
        name: "disabled duplicate",
        kind: CaseKind::NearMiss,
        build: inactive_duplicate,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::fires_without_winner(X, ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole copy replaced away",
        kind: CaseKind::Deletion,
        build: only_copy_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "winning copy unreadable",
        kind: CaseKind::Incomplete,
        build: winner_copy_unreadable,
        expect: Expectation::fires_without_winner(X, ResolutionGap::ParseIncomplete),
    },
];
