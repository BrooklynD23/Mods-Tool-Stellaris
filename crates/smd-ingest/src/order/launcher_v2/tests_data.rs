//! Data-invariant, schema and encoding tests for the launcher reader.

use std::fs;

use smd_core::OrderError;

use super::fixture::{Launcher, PosKind};
use super::tests::{IDS, codes_of, detail_of, order, source, world_with};
use super::*;
use crate::testutil::World;

#[test]
fn launcher_schema_missing_column() {
    let w = World::new();
    let ddl = "CREATE TABLE playsets_mods (playsetId TEXT, modId TEXT, position INTEGER);";
    let l = Launcher::with_join_table(&w.user, ddl);
    drop(l);
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(matches!(run.attempt.result, Err(OrderError::SchemaUnknown { .. })));
    let why = detail_of(&run, codes::LAUNCHER_SCHEMA_UNKNOWN);
    assert!(why.contains("missing column playsets_mods.enabled"), "{why}");
    assert!(
        why.contains("found:")
            && why.contains("playsets_mods(playsetId TEXT, modId TEXT, position INTEGER)"),
        "{why}"
    );
    assert!(why.contains("mods(id TEXT"), "{why}");
    assert!(codes_of(&run).contains(&codes::LAUNCHER_SNAPSHOT_UNVERIFIED));
}

#[test]
fn launcher_schema_retyped_column_is_named() {
    let w = World::new();
    let ddl =
        "CREATE TABLE playsets_mods (playsetId TEXT, modId TEXT, position BLOB, enabled BOOLEAN);";
    drop(Launcher::with_join_table(&w.user, ddl));
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(
        detail_of(&run, codes::LAUNCHER_SCHEMA_UNKNOWN)
            .contains("playsets_mods.position is declared BLOB")
    );
}

#[test]
fn launcher_multiple_active_refused() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.playset("p2", true, false);
    l.fill("p2", &[("111", true)]);
    drop(l);
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(run.attempt.result.is_err(), "the reader does not choose");
    assert!(codes_of(&run).contains(&codes::LAUNCHER_MULTIPLE_ACTIVE));
    let why = detail_of(&run, codes::LAUNCHER_MULTIPLE_ACTIVE);
    assert!(why.contains("p1") && why.contains("p2"), "{why}");
}

#[test]
fn a_removed_active_playset_does_not_count_and_none_active_is_refused() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.playset("old", true, true);
    l.fill("old", &[("222", true)]);
    let ok = source(&w, false, Hooks::default()).run(&w.locations());
    assert_eq!(order(&ok.attempt.result.expect("one live active playset")), IDS);
    l.conn.execute("UPDATE playsets SET isActive = 0", []).unwrap();
    let none = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(codes_of(&none).contains(&codes::LAUNCHER_NO_ACTIVE));
}

#[test]
fn launcher_position_string_and_integer() {
    let ids = ["333", "111", "222"];
    let mut orders = Vec::new();
    for kind in [PosKind::Int, PosKind::PaddedText] {
        let (w, l) = world_with(kind, &ids);
        let p =
            source(&w, false, Hooks::default()).run(&w.locations()).attempt.result.expect("reads");
        orders.push(order(&p).iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
        drop(l);
    }
    assert_eq!(orders[0], orders[1]);
    assert_eq!(orders[0], ["333", "111", "222"]);
}

#[test]
fn booleans_accept_text_and_other_values_are_schema_unknown() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.conn.execute("UPDATE playsets_mods SET enabled = 'false' WHERE modId = '222'", []).unwrap();
    let p = source(&w, false, Hooks::default()).run(&w.locations()).attempt.result.expect("reads");
    assert_eq!(p.entries.iter().map(|e| e.enabled).collect::<Vec<_>>(), [true, false, true]);
    l.conn.execute("UPDATE playsets_mods SET enabled = 'maybe' WHERE modId = '222'", []).unwrap();
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(matches!(run.attempt.result, Err(OrderError::SchemaUnknown { .. })));
}

#[test]
fn duplicate_positions_and_dangling_mod_ids_are_refused() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.conn.execute("UPDATE playsets_mods SET position = 10", []).unwrap();
    assert!(source(&w, false, Hooks::default()).run(&w.locations()).attempt.result.is_err());
    let (w2, l2) = world_with(PosKind::Int, &IDS);
    l2.conn.execute("INSERT INTO playsets_mods VALUES ('p1', 'ghost', 99, 1)", []).unwrap();
    let run = source(&w2, false, Hooks::default()).run(&w2.locations());
    assert!(detail_of(&run, codes::LAUNCHER_SNAPSHOT_UNVERIFIED).contains("ghost"));
}

#[test]
fn an_absent_is_removed_column_means_the_older_schema() {
    let w = World::new();
    w.workshop_mod("111", "111");
    let conn = Connection::open(w.user.join(FILE)).unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         CREATE TABLE mods (id TEXT, steamId TEXT);
         CREATE TABLE playsets (id TEXT, name TEXT, isActive BOOLEAN);
         CREATE TABLE playsets_mods (playsetId TEXT, modId TEXT, position TEXT, enabled BOOLEAN);
         INSERT INTO mods VALUES ('m', '111');
         INSERT INTO playsets VALUES ('p', 'old', 1);
         INSERT INTO playsets_mods VALUES ('p', 'm', '0000000001', 1);",
    )
    .unwrap();
    let p = source(&w, false, Hooks::default()).run(&w.locations()).attempt.result.expect("reads");
    assert_eq!(order(&p), ["111"]);
    drop(conn);
}

#[test]
fn no_launcher_file_is_not_present_and_garbage_is_unverified() {
    let w = World::new();
    let src = source(&w, true, Hooks::default());
    assert_eq!(src.run(&w.locations()).attempt.result, Err(OrderError::NotPresent));
    fs::write(w.user.join(FILE), b"definitely not sqlite, but long enough to read a header from ........................................................................").unwrap();
    let run = src.run(&w.locations());
    assert!(detail_of(&run, codes::LAUNCHER_SNAPSHOT_UNVERIFIED).contains("header"));
}

#[test]
fn header_parsing_reads_the_wal_flag() {
    let mut h = [0u8; 100];
    h[..16].copy_from_slice(b"SQLite format 3\0");
    let zero = header::Header { wal: false, schema_cookie: 0, user_version: 0, application_id: 0 };
    assert_eq!(header::parse(&h), Ok(zero));
    h[18] = 2;
    h[19] = 2;
    assert_eq!(header::parse(&h), Ok(header::Header { wal: true, ..zero }));
    h[40..44].copy_from_slice(&7u32.to_be_bytes());
    h[60..64].copy_from_slice(&9u32.to_be_bytes());
    let got = header::parse(&h).expect("parses");
    assert_eq!((got.schema_cookie, got.user_version), (7, 9));
    assert!(got.describe().contains("user_version 9"));
    h[0] = b'X';
    assert!(header::parse(&h).is_err());
}

#[test]
fn an_unverified_snapshot_names_the_launcher_database_identifiers() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.conn.execute_batch("PRAGMA user_version = 4242;").unwrap();
    l.conn.execute_batch("DROP TABLE playsets_mods;").unwrap();
    drop(l); // closing checkpoints into the main file, whose header carries user_version
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(run.attempt.result.is_err());
    let why = detail_of(&run, codes::LAUNCHER_SNAPSHOT_UNVERIFIED);
    assert!(why.contains("user_version 4242"), "{why}");
}
