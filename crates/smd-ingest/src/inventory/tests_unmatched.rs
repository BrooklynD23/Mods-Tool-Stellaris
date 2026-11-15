//! A playset entry no discovered mod matches must not turn the other mods `Inactive`.

use smd_core::Activation;

use super::*;
use crate::order::launcher_v2::fixture::{Launcher, PosKind};
use crate::order::{OrderChain, SourceOptions};
use crate::testutil::World;

fn activation_of(w: &World, key: &str) -> Activation {
    let chain =
        OrderChain::standard(&SourceOptions { discovery: w.options(), ..SourceOptions::default() });
    let inv = inventory(&w.locations(), &chain, &w.options());
    let (_, meta) = inv
        .table
        .iter()
        .find(|(_, m)| m.key.as_str().trim_start_matches("steam:") == key)
        .expect("in the table");
    meta.activation
}

fn world(with_unmatched_entry: bool) -> World {
    let w = World::new();
    w.workshop_mod("111", "Listed");
    w.workshop_mod("222", "Unlisted");
    let l = Launcher::create(&w.user, PosKind::Int);
    l.mod_row_with("a", &[("steamId", "111")]);
    l.playset("p1", true, false);
    if with_unmatched_entry {
        l.mod_row_with("ghost", &[("steamId", "999")]);
        l.fill("p1", &[("a", true), ("ghost", true)]);
    } else {
        l.fill("p1", &[("a", true)]);
    }
    l.checkpoint();
    w
}

#[test]
fn without_an_unmatched_entry_an_unlisted_mod_is_inactive() {
    let w = world(false);
    assert_eq!(activation_of(&w, "111"), Activation::Active);
    assert_eq!(activation_of(&w, "222"), Activation::Inactive);
}

#[test]
fn with_an_unmatched_entry_an_unlisted_mod_is_unknown_not_inactive() {
    let w = world(true);
    assert_eq!(activation_of(&w, "111"), Activation::Active);
    assert_eq!(activation_of(&w, "222"), Activation::Unknown);
}
