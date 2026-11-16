//! Each key a playset entry is joined to a discovered mod by, and their precedence.

use std::fs;
use std::path::PathBuf;

use super::fixture::{Launcher, PosKind};
use super::join::{self, Ident};
use super::rows::ModRow;
use super::tests::source;
use super::*;
use crate::testutil::World;

fn ident(file: Option<&str>, remote: Option<&str>, paths: &[&str]) -> Ident {
    Ident {
        mod_file: file.map(str::to_owned),
        remote_id: remote.map(str::to_owned),
        evidenced: true,
        paths: paths.iter().map(PathBuf::from).collect(),
    }
}

fn row(f: impl FnOnce(&mut ModRow)) -> ModRow {
    let mut r = ModRow { id: "1".into(), ..ModRow::default() };
    f(&mut r);
    r
}

fn some(s: &str) -> Option<String> {
    Some(s.to_owned())
}

#[test]
fn every_key_joins_alone() {
    let idents = [
        ident(Some("a.mod"), None, &[]),
        ident(None, Some("steam:200"), &[]),
        ident(None, Some("pdx:300"), &[]),
        ident(None, None, &["/m/dir"]),
        ident(None, None, &["/m/z.zip"]),
    ];
    let base = Path::new("/u");
    let cases = [
        (row(|r| r.game_registry_id = some("mod/A.mod")), Some(0)),
        (row(|r| r.game_registry_id = some("mod\\a.mod")), Some(0)),
        (row(|r| r.steam_id = some("200")), Some(1)),
        (row(|r| r.pdx_id = some("300")), Some(2)),
        (row(|r| r.dir_path = some("/m/dir")), Some(3)),
        (row(|r| r.archive_path = some("/m/z.zip")), Some(4)),
        (row(|r| r.steam_id = some("999")), None),
        (row(|_| {}), None),
    ];
    for (r, want) in cases {
        assert_eq!(join::find(&r, &idents, base), want, "{r:?}");
    }
}

#[test]
fn an_unevidenced_steam_id_also_meets_a_pdx_id_row() {
    let bare = Ident { evidenced: false, ..ident(Some("x.mod"), Some("steam:300"), &[]) };
    let strict = ident(None, Some("steam:300"), &[]);
    let r = row(|r| r.pdx_id = some("300"));
    assert_eq!(join::find(&r, &[bare], Path::new("/u")), Some(0));
    assert_eq!(join::find(&r, &[strict], Path::new("/u")), None, "evidenced Steam stays Steam");
}

#[test]
fn precedence_is_registry_then_steam_then_pdx_then_path() {
    let idents = [
        ident(None, None, &["/m/dir"]),
        ident(None, Some("steam:7"), &[]),
        ident(Some("a.mod"), None, &[]),
    ];
    let all = row(|r| {
        r.game_registry_id = some("mod/a.mod");
        r.steam_id = some("7");
        r.dir_path = some("/m/dir");
    });
    assert_eq!(join::find(&all, &idents, Path::new("/u")), Some(2));
    let no_registry = ModRow { game_registry_id: None, ..all.clone() };
    assert_eq!(join::find(&no_registry, &idents, Path::new("/u")), Some(1));
    let only_path = ModRow { steam_id: None, ..no_registry };
    assert_eq!(join::find(&only_path, &idents, Path::new("/u")), Some(0));
}

#[test]
fn a_launcher_path_matches_across_spelling_and_a_relative_one_against_the_user_dir() {
    let w = World::new();
    let dir = w.workshop.join("1");
    fs::create_dir_all(&dir).unwrap();
    let idents = [Ident { paths: vec![dir.clone()], ..Ident::default() }];
    let spelled = row(|r| r.dir_path = Some(format!("{}/./../1/", dir.display())));
    assert_eq!(join::find(&spelled, &idents, &w.user), Some(0));
    let relative = row(|r| r.dir_path = some("../workshop/1"));
    assert_eq!(join::find(&relative, &idents, &w.user), Some(0));
    let elsewhere = row(|r| r.dir_path = some("../workshop/2"));
    assert_eq!(join::find(&elsewhere, &idents, &w.user), None);
}

/// A world with a launcher file `alpha.mod` (dir under `mod/`), a workshop mod `2001`, a
/// Paradox Mods mod `3001`,
/// and an archive mod `zipped.mod` -> `mod/zipped.zip`.
fn world() -> (World, PathBuf, PathBuf, PathBuf) {
    let w = World::new();
    let alpha = w.local_mod("alpha", "Alpha");
    let ws = w.workshop_mod("2001", "Beta");
    w.paradox_mod("3001", "Gamma");
    let zip = w.user.join("mod").join("zipped.zip");
    fs::write(&zip, b"PK").unwrap();
    w.mod_file("zipped", "name=\"Zipped\"\narchive=\"mod/zipped.zip\"\n");
    (w, alpha, ws, zip)
}

#[test]
fn a_playset_entry_is_placed_by_each_key_end_to_end() {
    let (w, alpha, ws, zip) = world();
    let (alpha, ws, zip) = (
        alpha.to_string_lossy().into_owned(),
        ws.to_string_lossy().into_owned(),
        zip.to_string_lossy().into_owned(),
    );
    let cols: [&[(&str, &str)]; 6] = [
        &[("gameRegistryId", "mod/alpha.mod")],
        &[("steamId", "2001")],
        &[("pdxId", "3001")],
        &[("dirPath", &alpha)],
        &[("dirPath", &ws)],
        &[("archivePath", &zip)],
    ];
    for c in cols {
        let l = Launcher::create(&w.user, PosKind::Int);
        l.mod_row_with("only", c);
        l.playset("p1", true, false);
        l.fill("p1", &[("only", true)]);
        let run = source(&w, false, Hooks::default()).run(&w.locations());
        let p = run.attempt.result.unwrap_or_else(|e| panic!("{c:?}: {e:?}"));
        assert_eq!((p.entries.len(), p.unmatched), (1, 0), "{c:?}: {:?}", run.attempt.diagnostics);
        drop(l);
        fs::remove_file(w.user.join(FILE)).unwrap();
        for ext in ["-wal", "-shm"] {
            let _ = fs::remove_file(w.user.join(format!("{FILE}{ext}")));
        }
    }
}

#[test]
fn an_entry_that_matches_nothing_is_counted_and_reported() {
    let (w, ..) = world();
    let l = Launcher::create(&w.user, PosKind::Int);
    l.mod_row_with("ghost", &[("steamId", "424242"), ("displayName", "Ghost")]);
    l.playset("p1", true, false);
    l.fill("p1", &[("ghost", true)]);
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    let p = run.attempt.result.expect("reads");
    assert_eq!((p.entries.len(), p.unmatched), (0, 1));
    assert!(
        run.attempt
            .diagnostics
            .iter()
            .any(|d| d.code == codes::PLAYSET_MOD_UNMATCHED && &*d.subject == "Ghost")
    );
}
