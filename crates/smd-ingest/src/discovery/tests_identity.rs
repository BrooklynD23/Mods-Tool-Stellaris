//! Mod identity: platform-namespaced keys, keys that survive a version bump, and the
//! launcher database being read once per scan.

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use smd_core::SourceRole;

use super::*;
use crate::order::launcher_v2::fixture::{Launcher, PosKind};
use crate::order::{LauncherV2Options, OrderChain, SourceOptions};
use crate::testutil::World;

fn keys(d: &Discovery) -> Vec<&str> {
    d.mods.iter().map(|m| m.key.as_str()).collect()
}

#[test]
fn the_same_number_on_two_platforms_is_two_mods() {
    let w = World::new();
    w.workshop_mod("500", "Steam One");
    w.paradox_mod("500", "Paradox One");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(keys(&d), ["pdx:500", "steam:500"]);
    let role = |k: &str| d.mods.iter().find(|m| m.key.as_str() == k).unwrap().role;
    assert_eq!(role("steam:500"), SourceRole::Workshop);
    assert_eq!(role("pdx:500"), SourceRole::ParadoxMods);
}

#[test]
fn an_id_with_no_platform_evidence_is_steam_compatible() {
    let w = World::new();
    w.mod_file("whatever", "name=\"W\"\nremote_file_id=\"77\"\n");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(keys(&d), ["steam:77"]);
}

#[test]
fn a_version_bump_does_not_rekey_a_local_mod() {
    let w = World::new();
    let key_at = |version: &str| {
        w.mod_file("local", &format!("name=\"L\"\nversion=\"{version}\"\n"));
        keys(&discover(&w.locations(), &w.options()))[0].to_owned()
    };
    let (before, after) = (key_at("1.0"), key_at("1.1"));
    assert!(before.starts_with("h-"));
    assert_eq!(before, after);
}

#[test]
fn a_db_row_with_only_a_pdx_id_is_a_paradox_mod_and_a_steam_id_wins() {
    let w = World::new();
    let l = Launcher::create(&w.user, PosKind::Int);
    l.playset("p1", true, false);
    for (row, ids) in [("a", ("pdxId", "9")), ("b", ("steamId", "9"))] {
        let dir = w.tmp.path().join("elsewhere").join(row);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("descriptor.mod"), format!("name=\"M{row}\"\n")).unwrap();
        l.mod_row_with(row, &[("dirPath", &dir.to_string_lossy()), ids]);
    }
    let d = discover(&w.locations(), &w.options());
    assert_eq!(keys(&d), ["pdx:9", "steam:9"]);
    let a = d.mods.iter().find(|m| &*m.name == "Ma").unwrap();
    assert_eq!(a.role, SourceRole::ParadoxMods);
}

#[test]
fn a_pdx_id_does_not_join_a_steam_mod_of_the_same_digits() {
    let w = World::new();
    w.workshop_mod("500", "Steam One");
    let l = Launcher::create(&w.user, PosKind::Int);
    l.mod_row_with("x", &[("pdxId", "500"), ("displayName", "Other")]);
    l.playset("p1", true, false);
    l.fill("p1", &[("x", true)]);
    let chain = OrderChain::standard(&SourceOptions {
        discovery: w.options(),
        launcher: LauncherV2Options::default(),
    });
    let found = discover(&w.locations(), &w.options());
    let p = chain.run_with(&w.locations(), &found).playset;
    assert_eq!((p.entries.len(), p.unmatched), (0, 1), "pdx:500 is not steam:500");
}

#[test]
fn the_launcher_database_is_read_once_per_scan_and_hooks_reach_that_read() {
    let w = World::new();
    w.workshop_mod("111", "One");
    let l = Launcher::create(&w.user, PosKind::Int);
    l.seed(&["111"]);
    l.checkpoint();
    let reads = Arc::new(AtomicU32::new(0));
    let counter = Arc::clone(&reads);
    let mut launcher = LauncherV2Options {
        allow_online: false,
        backoff: Duration::from_millis(5),
        ..LauncherV2Options::default()
    };
    launcher.hooks.between_passes = Some(Arc::new(move |_| {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    let chain = OrderChain::standard(&SourceOptions { discovery: w.options(), launcher });
    let inv = crate::inventory(&w.locations(), &chain, &w.options());
    assert_eq!(inv.provenance.order_source, "launcher_v2_sqlite");
    assert_eq!(reads.load(Ordering::SeqCst), 1, "one snapshot, shared by discovery and order");
}
