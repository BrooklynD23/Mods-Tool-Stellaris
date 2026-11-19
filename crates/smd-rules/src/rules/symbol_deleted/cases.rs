//! The fixtures `core.symbol_deleted` is defined by.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase, WinnerExpect};

const V: &str = "common/x/00_v.txt";

/// Fires naming no winner and carrying no gap: a stated deletion.
const DELETED: Expectation =
    Expectation::Fires { path: "X", winner: WinnerExpect::NoWinner, gap: None };

fn replaced_without_it(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.define(SourceId::VANILLA, V, &[("X", "v")]).define(a, V, &[("Y", "a")]);
}

fn replaced_with_it(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.define(SourceId::VANILLA, V, &[("X", "v")]).define(a, V, &[("X", "a")]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    replaced_without_it(b);
    b.directory_scan();
}

fn replace_path_drop(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.define(SourceId::VANILLA, V, &[("X", "v")]).replace_path(a, "common/x");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn replacement_unparsed(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.define(SourceId::VANILLA, V, &[("X", "v")]);
    b.file_with(a, V, FileShape::unread(UnparsedReason::Oversize));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "file replaced by a copy without the symbol",
        kind: CaseKind::Positive,
        build: replaced_without_it,
        expect: DELETED,
    },
    RuleCase {
        name: "replacement still defines it",
        kind: CaseKind::NearMiss,
        build: replaced_with_it,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source: both copies may survive",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::None,
    },
    RuleCase {
        name: "sole definer dropped by replace_path",
        kind: CaseKind::Deletion,
        build: replace_path_drop,
        expect: DELETED,
    },
    RuleCase {
        name: "replacing copy over the size cap",
        kind: CaseKind::Incomplete,
        build: replacement_unparsed,
        expect: Expectation::fires_without_winner("X", ResolutionGap::ParseIncomplete),
    },
];
