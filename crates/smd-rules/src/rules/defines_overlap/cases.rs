//! The fixtures `pdx.defines_overlap` is defined by. Fixture directories are LIOS with a
//! basename filename key unless a case says otherwise.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase};

const A: &str = "common/defines/a.txt";
const B: &str = "common/defines/b.txt";
const KEY: &str = "NGameplay.MAX_X";

fn two_mods_one_key(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.defines(a, A, &[(KEY, "1")]).defines(m, B, &[(KEY, "2")]);
}

fn different_keys(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.defines(a, A, &[(KEY, "1")]).defines(m, B, &[("NGameplay.MAX_Y", "2")]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.defines(a, A, &[(KEY, "1")]).defines(m, A, &[(KEY, "2")]);
    b.directory_scan();
}

fn sole_site_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.defines(SourceId::VANILLA, A, &[(KEY, "v")]).replace_path(a, "common/defines");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn later_file_unparsed(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    let c = b.add_mod("C", &[2, 2], Activation::Active);
    b.defines(a, A, &[(KEY, "1")]).defines(m, B, &[(KEY, "2")]);
    b.file_with(c, "common/defines/c.txt", FileShape::unread(UnparsedReason::Oversize));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "two mods, one define key",
        kind: CaseKind::Positive,
        build: two_mods_one_key,
        expect: Expectation::fires_on_path(KEY, "B"),
    },
    RuleCase {
        name: "two defines files, different keys",
        kind: CaseKind::NearMiss,
        build: different_keys,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::fires_without_winner(KEY, ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole site replaced away",
        kind: CaseKind::Deletion,
        build: sole_site_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "a later-sorting defines file was not parsed",
        kind: CaseKind::Incomplete,
        build: later_file_unparsed,
        expect: Expectation::fires_without_winner(KEY, ResolutionGap::ParseIncomplete),
    },
];
