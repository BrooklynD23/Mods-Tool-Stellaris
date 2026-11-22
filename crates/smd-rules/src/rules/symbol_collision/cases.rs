//! The fixtures `core.symbol_collision` is defined by. Fixture directories are LIOS with a
//! basename filename key unless a case says otherwise.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase};

const A: &str = "common/x/a.txt";
const B: &str = "common/x/b.txt";

fn two_overhauls(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.define(a, A, &[("X", "1")]).define(m, B, &[("X", "2")]);
}

fn inactive_duplicate(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let off = b.add_mod("Off", &[2, 1], Activation::Inactive);
    b.define(a, A, &[("X", "1")]).define(off, B, &[("X", "2")]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.define(a, A, &[("X", "1")]).define(m, A, &[("X", "2")]);
    b.directory_scan();
}

fn sole_definer_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.define(SourceId::VANILLA, A, &[("X", "v")]).replace_path(a, "common/x");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn later_file_unparsed(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    let c = b.add_mod("C", &[2, 2], Activation::Active);
    b.define(a, A, &[("X", "1")]).define(m, B, &[("X", "2")]);
    b.file_with(c, "common/x/c.txt", FileShape::unread(UnparsedReason::Oversize));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "two overhauls, one symbol",
        kind: CaseKind::Positive,
        build: two_overhauls,
        expect: Expectation::fires_on_path("X", "B"),
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
        expect: Expectation::fires_without_winner("X", ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole definer replaced away",
        kind: CaseKind::Deletion,
        build: sole_definer_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "a later-sorting file was not parsed",
        kind: CaseKind::Incomplete,
        build: later_file_unparsed,
        expect: Expectation::fires_without_winner("X", ResolutionGap::ParseIncomplete),
    },
];
