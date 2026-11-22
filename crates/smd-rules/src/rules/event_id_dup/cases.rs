//! The fixtures `pdx.event_id_dup` is defined by. Fixture directories are LIOS with a
//! basename filename key unless a case says otherwise.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase};

const A: &str = "events/a.txt";
const B: &str = "events/b.txt";

fn two_mods_one_id(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.events(a, A, Some("ns"), &["ns.1"]).events(m, B, Some("ns"), &["ns.1"]);
}

fn different_ordinals(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    b.events(a, A, Some("ns"), &["ns.1"]).events(m, B, Some("ns"), &["ns.2"]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.events(a, A, None, &["ns.1"]).events(m, A, None, &["ns.1"]);
    b.directory_scan();
}

fn sole_site_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.events(SourceId::VANILLA, A, Some("ns"), &["ns.1"]).replace_path(a, "events");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

fn later_file_unparsed(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    let c = b.add_mod("C", &[2, 2], Activation::Active);
    b.events(a, A, None, &["ns.1"]).events(m, B, None, &["ns.1"]);
    b.file_with(c, "events/c.txt", FileShape::unread(UnparsedReason::Oversize));
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "two mods, one event id",
        kind: CaseKind::Positive,
        build: two_mods_one_id,
        expect: Expectation::fires_on_path("ns.1", "B"),
    },
    RuleCase {
        name: "shared namespace, different ordinals",
        kind: CaseKind::NearMiss,
        build: different_ordinals,
        expect: Expectation::None,
    },
    RuleCase {
        name: "no order source",
        kind: CaseKind::OrderUnknown,
        build: no_order_source,
        expect: Expectation::fires_without_winner("ns.1", ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole site replaced away",
        kind: CaseKind::Deletion,
        build: sole_site_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "a later-sorting events file was not parsed",
        kind: CaseKind::Incomplete,
        build: later_file_unparsed,
        expect: Expectation::fires_without_winner("ns.1", ResolutionGap::ParseIncomplete),
    },
];
