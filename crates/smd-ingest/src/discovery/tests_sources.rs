//! The sources beyond `*.mod` files and workshop directories: unregistered directories,
//! loose archives, and rows only the launcher database has.

use std::fs;
use std::path::Path;

use smd_core::SourceRole;

use super::*;
use crate::codes;
use crate::order::launcher_v2::fixture::{Launcher, PosKind};
use crate::testutil::World;

fn find<'a>(d: &'a Discovery, name: &str) -> &'a DiscoveredMod {
    let names: Vec<&str> = d.mods.iter().map(|m| &*m.name).collect();
    d.mods.iter().find(|m| &*m.name == name).unwrap_or_else(|| panic!("{name} not in {names:?}"))
}

fn code_count(d: &Discovery, code: &str) -> usize {
    d.diagnostics.iter().filter(|x| x.code == code).count()
}

fn handmade(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("descriptor.mod"), format!("name=\"{name}\"\n")).unwrap();
}

#[test]
fn an_unregistered_directory_is_found_readable_and_warned() {
    let w = World::new();
    w.local_mod("alpha", "Alpha");
    handmade(&w.user.join("mod").join("handmade"), "Handmade");
    fs::create_dir_all(w.user.join("mod").join("junk")).unwrap();
    fs::write(w.user.join("mod").join("junk").join("notes.txt"), "x").unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 2, "junk without a descriptor.mod is not a mod");
    let m = find(&d, "Handmade");
    assert_eq!(m.origin, ModOrigin::UnregisteredDir);
    assert_eq!(
        (m.content, m.role, m.mod_file.as_deref()),
        (ContentState::Readable, SourceRole::Local, None)
    );
    assert_eq!(m.content_root.as_deref(), Some(w.user.join("mod").join("handmade").as_path()));
    assert_eq!(find(&d, "Alpha").origin, ModOrigin::ModFile, "a claimed directory is not re-found");
    let warn = d.diagnostics.iter().find(|x| x.code == codes::MOD_UNREGISTERED).expect("warned");
    assert_eq!(&*warn.subject, "mod/handmade");
    assert_eq!(code_count(&d, codes::MOD_UNREGISTERED), 1);
    assert!(!d.incomplete);
}

#[test]
fn a_directory_a_launcher_file_points_at_by_another_spelling_is_claimed() {
    let w = World::new();
    let dir = w.user.join("mod").join("moved");
    handmade(&dir, "Moved");
    // `path=` through a `..` hop and with a trailing slash still names the same directory.
    let spelled = format!("{}/../moved/", dir.display());
    w.mod_file("moved", &format!("name=\"Moved\"\npath=\"{spelled}\"\n"));
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1, "{:?}", d.mods.iter().map(|m| &m.origin).collect::<Vec<_>>());
    assert_eq!(code_count(&d, codes::MOD_UNREGISTERED), 0);
}

#[test]
fn a_loose_archive_is_inventoried_unread_and_a_referenced_one_is_not_repeated() {
    let w = World::new();
    fs::write(w.user.join("mod").join("loose.zip"), b"PK").unwrap();
    fs::write(w.user.join("mod").join("used.zip"), b"PK").unwrap();
    w.mod_file("used", "name=\"Used\"\narchive=\"mod/used.zip\"\n");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 2, "{:?}", d.mods.iter().map(|m| &m.name).collect::<Vec<_>>());
    let loose = find(&d, "loose");
    assert_eq!(loose.origin, ModOrigin::LooseArchive);
    assert_eq!(loose.content, ContentState::Unread(UnreadReason::Archive));
    assert_eq!((loose.role, loose.content_root.is_none()), (SourceRole::Archive, true));
    assert_eq!(find(&d, "Used").origin, ModOrigin::ModFile);
}

fn db_world() -> (World, Launcher) {
    let w = World::new();
    let l = Launcher::create(&w.user, PosKind::Int);
    l.playset("p1", true, false);
    (w, l)
}

#[test]
fn launcher_db_only_rows_become_mods_and_claimed_rows_do_not() {
    let (w, l) = db_world();
    let paradox = w.tmp.path().join("elsewhere").join("paradox_mod");
    handmade(&paradox, "Paradox Download");
    let alpha = w.local_mod("alpha", "Alpha");
    l.mod_row_with("r1", &[("dirPath", &paradox.to_string_lossy()), ("displayName", "PD")]);
    l.mod_row_with("r2", &[("dirPath", &alpha.to_string_lossy())]);
    l.mod_row_with("r3", &[("gameRegistryId", "mod/alpha.mod")]);
    l.mod_row_with("r4", &[("displayName", "no path at all")]);
    let d = discover(&w.locations(), &w.options());
    assert_eq!(
        d.mods.len(),
        2,
        "{:?}",
        d.mods.iter().map(|m| (&m.name, m.origin)).collect::<Vec<_>>()
    );
    let m = find(&d, "Paradox Download");
    assert_eq!(m.origin, ModOrigin::LauncherDbOnly);
    assert_eq!(m.content, ContentState::Readable);
    assert_eq!(m.content_root.as_deref(), Some(paradox.canonicalize().unwrap().as_path()));
    assert_eq!(code_count(&d, codes::LAUNCHER_DB_NOT_CONSULTED), 0);
    assert!(!d.incomplete);
}

#[test]
fn launcher_db_paths_go_through_the_refuse_guard_and_missing_ones_are_inventoried() {
    let (w, l) = db_world();
    l.mod_row_with(
        "r1",
        &[("dirPath", &w.home.to_string_lossy()), ("displayName", "Swallows home")],
    );
    l.mod_row_with(
        "r2",
        &[("dirPath", &w.tmp.path().join("gone").to_string_lossy()), ("displayName", "Gone")],
    );
    l.mod_row_with(
        "r3",
        &[
            ("archivePath", &w.tmp.path().join("a.zip").to_string_lossy()),
            ("displayName", "Zipped"),
        ],
    );
    let d = discover(&w.locations(), &w.options());
    let by = |n: &str| find(&d, n);
    let home = w.home.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(by(&home).content, ContentState::Unread(UnreadReason::RootRefused));
    assert_eq!(by("gone").content, ContentState::Unread(UnreadReason::PathMissing));
    let z = by("a.zip");
    assert_eq!(
        (z.content, z.role),
        (ContentState::Unread(UnreadReason::Archive), SourceRole::Archive)
    );
    assert!(d.mods.iter().all(|m| m.origin == ModOrigin::LauncherDbOnly));
    assert_eq!(code_count(&d, codes::ROOT_REFUSED), 1);
}

#[test]
fn no_launcher_db_is_said_not_consulted_and_is_not_incompleteness() {
    let w = World::new();
    w.local_mod("alpha", "Alpha");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(code_count(&d, codes::LAUNCHER_DB_NOT_CONSULTED), 1);
    assert!(!d.incomplete);
}

#[test]
fn an_unreadable_launcher_db_is_said_not_consulted_with_the_reason() {
    let w = World::new();
    w.write_user("launcher-v2.sqlite", b"this is not a database at all, not even close, no.");
    let d = discover(&w.locations(), &w.options());
    let note =
        d.diagnostics.iter().find(|x| x.code == codes::LAUNCHER_DB_NOT_CONSULTED).expect("said so");
    assert!(note.detail.contains("not looked for"), "{}", note.detail);
    assert!(!d.incomplete, "{:?}", d.diagnostics);
}
