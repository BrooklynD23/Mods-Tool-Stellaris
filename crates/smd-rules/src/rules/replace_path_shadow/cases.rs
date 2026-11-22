//! The fixtures `pdx.replace_path_shadow` is defined by. The subject text of a
//! declaration is `<mod key>: <prefix>`.

use smd_core::{Activation, ResolutionGap, SourceId};
use smd_index::{PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase, WinnerExpect};

const SUBJECT: &str = "mod:B: common/x";

fn drops_an_earlier_mod(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, "common/x/a.txt", "a").file(SourceId::VANILLA, "common/x/v.txt", "v");
    b.replace_path(m, "common/x").replace_scope(ReplacePathScope::VanillaAndEarlier);
}

fn drops_vanilla_only(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, "common/y/a.txt", "a").file(SourceId::VANILLA, "common/x/v.txt", "v");
    b.replace_path(m, "common/x").replace_scope(ReplacePathScope::VanillaAndEarlier);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    drops_an_earlier_mod(b);
    b.directory_scan();
}

fn sole_definer_dropped(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.define(a, "common/x/a.txt", &[("X", "1")]).replace_path(m, "common/x");
    b.replace_scope(ReplacePathScope::VanillaAndEarlier);
}

fn dropped_mod_content_incomplete(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.file(a, "common/x/a.txt", "a").content_incomplete(a).replace_path(m, "common/x");
    b.replace_scope(ReplacePathScope::VanillaAndEarlier);
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "a later mod's replace_path drops an earlier mod's file",
        kind: CaseKind::Positive,
        build: drops_an_earlier_mod,
        expect: Expectation::Fires { path: SUBJECT, winner: WinnerExpect::NoWinner, gap: None },
    },
    RuleCase {
        name: "replace_path over vanilla files only",
        kind: CaseKind::NearMiss,
        build: drops_vanilla_only,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::fires_without_winner(SUBJECT, ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "the only definer's file is dropped",
        kind: CaseKind::Deletion,
        build: sole_definer_dropped,
        expect: Expectation::Fires { path: SUBJECT, winner: WinnerExpect::NoWinner, gap: None },
    },
    RuleCase {
        name: "the dropped mod was not fully read",
        kind: CaseKind::Incomplete,
        build: dropped_mod_content_incomplete,
        expect: Expectation::fires_without_winner(SUBJECT, ResolutionGap::ParseIncomplete),
    },
];
