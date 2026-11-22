//! The fixtures `pdx.namespace_share` is defined by.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase, WinnerExpect};

const A: &str = "events/a.txt";
const B: &str = "events/b.txt";

fn two_mods_one_namespace(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.events(a, A, Some("ns"), &["ns.1"]).events(m, B, Some("ns"), &["ns.2"]);
}

fn shared_with_vanilla(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.events(SourceId::VANILLA, A, Some("ns"), &["ns.1"]).events(a, B, Some("ns"), &["ns.2"]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.events(a, A, Some("ns"), &[]).events(m, A, Some("ns"), &[]);
    b.directory_scan();
}

fn sole_declarer_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.events(SourceId::VANILLA, A, Some("ns"), &["ns.1"]).replace_path(a, "events");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn third_file_unparsed(b: &mut PlaysetBuilder) {
    two_mods_one_namespace(b);
    let c = b.add_mod("C", &[2, 2], Activation::Active);
    b.file_with(c, "events/c.txt", FileShape::unread(UnparsedReason::Oversize));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "two unrelated mods, one namespace",
        kind: CaseKind::Positive,
        build: two_mods_one_namespace,
        expect: Expectation::Fires { path: "ns", winner: WinnerExpect::NoWinner, gap: None },
    },
    RuleCase {
        name: "a mod sharing vanilla's namespace",
        kind: CaseKind::NearMiss,
        build: shared_with_vanilla,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::fires_without_winner("ns", ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole declarer replaced away",
        kind: CaseKind::Deletion,
        build: sole_declarer_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "a third events file was not parsed",
        kind: CaseKind::Incomplete,
        build: third_file_unparsed,
        expect: Expectation::fires_without_winner("ns", ResolutionGap::ParseIncomplete),
    },
];
