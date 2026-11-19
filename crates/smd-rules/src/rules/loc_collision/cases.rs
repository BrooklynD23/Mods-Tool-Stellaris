//! The fixtures `core.loc_collision` is defined by. LIOS, basename filename key; paths
//! under `localisation/replace/` take tier 1 where a case says so.

use smd_core::{Activation, ResolutionGap, SourceId, UnparsedReason};
use smd_index::{FileShape, PlaysetBuilder, ReplacePathScope};

use crate::case::{CaseKind, Expectation, RuleCase};

const REPLACE: &str = "localisation/replace/";

/// The doc 07 worked case: the earlier mod's replace entry beats the later mod's plain
/// one, although load order and filename both favour the later mod.
fn replace_tier_beats_order(b: &mut PlaysetBuilder) {
    let early = b.add_mod("B", &[2, 0], Activation::Active);
    let late = b.add_mod("A", &[2, 1], Activation::Active);
    b.localise(late, "localisation/z.yml", &[("K", "from a")]);
    b.localise(early, "localisation/replace/a.yml", &[("K", "from b")]);
    b.tier_under(REPLACE);
}

fn inactive_duplicate(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let off = b.add_mod("Off", &[2, 1], Activation::Inactive);
    b.localise(a, "localisation/a.yml", &[("K", "1")]);
    b.localise(off, "localisation/b.yml", &[("K", "2")]);
}

fn no_order_source(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[], Activation::Unknown);
    let m = b.add_mod("B", &[], Activation::Unknown);
    b.localise(a, "localisation/a.yml", &[("K", "1")]);
    b.localise(m, "localisation/a.yml", &[("K", "2")]);
    b.directory_scan();
}

fn sole_entry_replaced_away(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    b.localise(SourceId::VANILLA, "localisation/a.yml", &[("K", "v")]);
    b.replace_path(a, "localisation");
    b.replace_scope(ReplacePathScope::VanillaOnly);
}

/// A plain-tier winner exists among what was read, but an unread replace-tier file could
/// hold the key and would beat it.
fn replace_file_unparsed(b: &mut PlaysetBuilder) {
    let a = b.add_mod("A", &[2, 0], Activation::Active);
    let m = b.add_mod("B", &[2, 1], Activation::Active);
    let c = b.add_mod("C", &[2, 2], Activation::Active);
    b.localise(a, "localisation/a.yml", &[("K", "1")]);
    b.localise(m, "localisation/b.yml", &[("K", "2")]);
    b.file_with(c, "localisation/replace/c.yml", FileShape::unread(UnparsedReason::Oversize));
    b.tier_under(REPLACE);
}

pub(super) const CASES: &[RuleCase] = &[
    RuleCase {
        name: "replace tier beats load order and filename",
        kind: CaseKind::Positive,
        build: replace_tier_beats_order,
        expect: Expectation::fires_on_path("K", "B"),
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
        expect: Expectation::fires_without_winner("K", ResolutionGap::OrderUnknown),
    },
    RuleCase {
        name: "sole entry replaced away",
        kind: CaseKind::Deletion,
        build: sole_entry_replaced_away,
        expect: Expectation::None,
    },
    RuleCase {
        name: "an unread replace-tier file",
        kind: CaseKind::Incomplete,
        build: replace_file_unparsed,
        expect: Expectation::fires_without_winner("K", ResolutionGap::ParseIncomplete),
    },
];
