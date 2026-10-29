use std::fs;
use std::path::Path;
use std::time::Duration;

use smd_core::{Confidence, OrderError, Playset, SnapshotMethod};

use super::fixture::{Launcher, PosKind};
use super::*;
use crate::order::SourceOptions;
use crate::testutil::World;

pub(super) const IDS: [&str; 3] = ["111", "222", "333"];

pub(super) fn source(w: &World, allow_online: bool, hooks: Hooks) -> LauncherV2Sqlite {
    let launcher = LauncherV2Options {
        allow_online,
        backoff: Duration::from_millis(5),
        hooks,
        ..LauncherV2Options::default()
    };
    LauncherV2Sqlite::new(SourceOptions { discovery: w.options(), launcher })
}

pub(super) fn world_with(kind: PosKind, ids: &[&str]) -> (World, Launcher) {
    let w = World::new();
    for id in ids {
        w.workshop_mod(id, id);
    }
    let l = Launcher::create(&w.user, kind);
    l.seed(ids);
    (w, l)
}

pub(super) fn order(p: &Playset) -> Vec<&str> {
    p.entries.iter().map(|e| e.key.as_str()).collect()
}

pub(super) fn codes_of(run: &LauncherRun) -> Vec<&'static str> {
    run.attempt.diagnostics.iter().map(|d| d.code).collect()
}

pub(super) fn detail_of<'a>(run: &'a LauncherRun, code: &str) -> &'a str {
    &run.attempt.diagnostics.iter().find(|d| d.code == code).expect("diagnostic").detail
}

fn blake(p: &Path) -> Option<blake3::Hash> {
    fs::read(p).ok().map(|b| blake3::hash(&b))
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// A reorder committed to the `-wal` only, with the writer still open.
fn wal_only_reorder() -> (World, Launcher) {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.checkpoint();
    l.reorder(&["333", "111", "222"]);
    assert!(snapshot::wal_of(&l.path).metadata().unwrap().len() > 0, "the commit is in the -wal");
    (w, l)
}

/// What path A itself says about the fixture, for assertion messages: path A's failure
/// falls back to path B silently, so a mismatch alone does not say why.
fn why_not_online(w: &World) -> String {
    match snapshot::online(&w.user.join(super::FILE)) {
        Ok(_) => "path A opens the fixture fine".to_owned(),
        Err(f) => format!("path A fails at step {}: {}", f.step, f.detail),
    }
}

#[test]
fn a_naive_main_file_copy_would_return_the_old_order() {
    let (w, l) = wal_only_reorder();
    let copy = w.tmp.path().join("naive.sqlite");
    fs::copy(&l.path, &copy).unwrap();
    let c = Connection::open(&copy).unwrap();
    let first: String = c
        .query_row("SELECT modId FROM playsets_mods ORDER BY position LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(first, "111", "the fixture is only meaningful if main lacks the new order");
}

// Path A needs `mount::is_local` to recognise the temp dir, which it can only do on Linux
// (via /proc/self/mountinfo) and Windows (non-UNC path). On macOS there is no mount-type
// probe, so path B is always used and this test's premise does not hold.
#[test]
#[cfg(any(target_os = "linux", windows))]
fn h20_wal_only_commit_path_a() {
    let (w, l) = wal_only_reorder();
    assert!(mount::is_local(&w.user), "this test needs a local filesystem under the temp dir");
    let run = source(&w, true, Hooks::default()).run(&w.locations());
    let p = run.attempt.result.expect("path A answers");
    assert_eq!(order(&p), ["333", "111", "222"]);
    assert_eq!(p.snapshot, SnapshotMethod::OnlineBackup, "{}", why_not_online(&w));
    assert_eq!((p.order, p.activation), (Confidence::Exact, Confidence::Exact));
    assert_eq!(run.temp_dir, None, "path A makes no private directory");
    drop(l);
}

#[test]
fn h20_wal_only_commit_path_b() {
    let (w, l) = wal_only_reorder();
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    let p = run.attempt.result.expect("path B answers");
    assert_eq!(order(&p), ["333", "111", "222"]);
    assert_eq!(p.snapshot, SnapshotMethod::StableFileCopy);
    assert_eq!((p.order, p.activation), (Confidence::Exact, Confidence::Exact));
    let tmp = run.temp_dir.expect("path B used a private directory");
    assert!(!tmp.exists(), "the private directory is deleted on every exit path");
    drop(l);
}

#[test]
fn h20_torn_copy_fails_double_pass() {
    let (w, l) = wal_only_reorder();
    let wal = snapshot::wal_of(&l.path);
    let hook: Hook = Arc::new(move |_attempt| {
        use std::io::Write;
        let mut f = fs::OpenOptions::new().append(true).open(&wal).unwrap();
        f.write_all(b"x").unwrap();
    });
    let hooks = Hooks { between_passes: Some(hook), ..Hooks::default() };
    let run = source(&w, false, hooks).run(&w.locations());
    assert!(matches!(run.attempt.result, Err(OrderError::Unreadable { .. })));
    let why = detail_of(&run, codes::LAUNCHER_SNAPSHOT_UNVERIFIED);
    assert!(why.contains("double_pass") && why.contains("B (StableFileCopy)"), "{why}");
    assert!(!run.temp_dir.expect("temp dir").exists());
    drop(l);
}

/// Attempt 1 is torn while the launcher still has a `-wal`; before attempt 2 it commits
/// again, checkpoints and closes, which deletes the `-wal`. A `-wal` copied on attempt 1
/// must not be replayed over the checkpointed main file on attempt 2.
#[test]
fn h20_a_wal_copied_on_a_torn_attempt_is_not_replayed_on_the_next() {
    let (w, l) = wal_only_reorder();
    let wal = snapshot::wal_of(&l.path);
    let wal_in_hook = wal.clone();
    let tear: Hook = Arc::new(move |attempt| {
        use std::io::Write;
        if attempt == 1 {
            let mut f = fs::OpenOptions::new().append(true).open(&wal_in_hook).unwrap();
            f.write_all(b"x").unwrap();
        }
    });
    let writer = Arc::new(std::sync::Mutex::new(Some(l)));
    let w2 = Arc::clone(&writer);
    let wal_after = wal.clone();
    let close: Hook = Arc::new(move |attempt| {
        assert_eq!(attempt, 2);
        let l = w2.lock().unwrap().take().expect("writer still open");
        l.reorder(&["222", "333", "111"]);
        l.checkpoint();
        drop(l);
        assert!(!wal_after.exists(), "closing the last connection removes the -wal");
    });
    let hooks =
        Hooks { between_passes: Some(tear), between_attempts: Some(close), ..Hooks::default() };
    let run = source(&w, false, hooks).run(&w.locations());
    let p = run.attempt.result.expect("attempt 2 accepts");
    assert_eq!(order(&p), ["222", "333", "111"], "the checkpointed order, not the stale -wal's");
    assert_eq!(p.snapshot, SnapshotMethod::StableFileCopy);
}

#[test]
fn h20_corrupt_page_fails_integrity_check() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    let path = l.path.clone();
    drop(l); // the last connection closing checkpoints everything into the main file
    let mut bytes = fs::read(&path).unwrap();
    let page = usize::from(u16::from_be_bytes([bytes[16], bytes[17]]));
    assert!(bytes.len() >= page * 3, "fixture has several pages");
    for page_no in 2..=3 {
        let at = (page_no - 1) * page;
        bytes[at..at + page].fill(0xFF);
    }
    fs::write(&path, bytes).unwrap();
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert!(matches!(run.attempt.result, Err(OrderError::Corrupt { .. })), "{:?}", run.attempt);
    let why = detail_of(&run, codes::LAUNCHER_SNAPSHOT_UNVERIFIED);
    assert!(why.contains("integrity_check"), "{why}");
}

#[test]
fn h29_fixture_launcher_files_untouched() {
    // Path A is only reachable where `mount::is_local` can say yes (Linux, Windows); on
    // macOS path B is always taken, so only the path B pass is meaningful there.
    let modes: &[bool] =
        if cfg!(any(target_os = "linux", windows)) { &[true, false] } else { &[false] };
    for &allow_online in modes {
        let (w, l) = wal_only_reorder();
        let (main, wal) = (l.path.clone(), snapshot::wal_of(&l.path));
        let before = (blake(&main), blake(&wal), files_in(&w.user));
        assert!(before.1.is_some());
        let p = source(&w, allow_online, Hooks::default())
            .run(&w.locations())
            .attempt
            .result
            .expect("reads");
        let want = if allow_online {
            SnapshotMethod::OnlineBackup
        } else {
            SnapshotMethod::StableFileCopy
        };
        assert_eq!(p.snapshot, want, "{}", why_not_online(&w));
        let after = (blake(&main), blake(&wal), files_in(&w.user));
        assert_eq!(before.0, after.0, "main file bytes (online={allow_online})");
        assert_eq!(before.1, after.1, "-wal bytes (online={allow_online})");
        assert_eq!(
            before.2, after.2,
            "no file created beside the launcher's (online={allow_online})"
        );
        drop(l);
    }
}

// Exercises path A's live-digest comparison, which macOS never reaches (see above).
#[test]
#[cfg(any(target_os = "linux", windows))]
fn a_launcher_that_keeps_committing_is_reported_changing() {
    let (w, l) = wal_only_reorder();
    let writer = Arc::new(std::sync::Mutex::new(l));
    let w2 = Arc::clone(&writer);
    let hook: Hook = Arc::new(move |round| {
        let ids = if round % 2 == 0 { ["111", "222", "333"] } else { ["333", "222", "111"] };
        w2.lock().unwrap().reorder(&ids);
    });
    let hooks = Hooks { after_snapshot: Some(hook), ..Hooks::default() };
    let run = source(&w, true, hooks).run(&w.locations());
    assert!(run.attempt.result.is_ok());
    assert!(
        codes_of(&run).contains(&codes::LAUNCHER_CHANGING),
        "{:?}; {}",
        codes_of(&run),
        why_not_online(&w)
    );
}

#[test]
fn an_unmatched_mod_is_dropped_with_a_diagnostic() {
    let (w, l) = world_with(PosKind::Int, &IDS);
    l.mod_row("999");
    l.fill("p1", &[]);
    l.conn.execute("INSERT INTO playsets_mods VALUES ('p1', '999', 5, 1)", []).unwrap();
    let run = source(&w, false, Hooks::default()).run(&w.locations());
    assert_eq!(order(run.attempt.result.as_ref().expect("reads")), IDS);
    assert!(codes_of(&run).contains(&codes::PLAYSET_MOD_UNMATCHED));
}
