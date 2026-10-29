use std::fs;
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

use smd_core::{ContentIncompleteReason, ModKey, ParseCompleteness, UnparsedReason};

use super::*;
use crate::ReadOnlyRoot;
use crate::hash_reader;

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn walk_root(root: &Path, limits: &WalkLimits) -> WalkOutcome {
    ReadOnlyRoot::open(root).unwrap().walk(limits)
}

fn codes_of(o: &WalkOutcome) -> Vec<&'static str> {
    o.diagnostics.iter().map(|d| d.code).collect()
}

fn find<'a>(o: &'a WalkOutcome, key: &str) -> &'a WalkRecord {
    o.records.iter().find(|r| r.path.as_str() == key).unwrap_or_else(|| panic!("no {key}"))
}

#[test]
fn h22_file_symlink_refused() {
    let outside = tempfile::tempdir().unwrap();
    let canary = b"CANARY-7f3a91-never-read";
    put(outside.path(), "secret.txt", canary);
    let canary_blob = hash_reader(&canary[..], 1024).unwrap().0;
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "ok.txt", b"hello");
    symlink(outside.path().join("secret.txt"), root.path().join("link.txt")).unwrap();

    let out = walk_root(root.path(), &WalkLimits::default());
    assert_eq!(out.records.len(), 2);
    assert!(find(&out, "ok.txt").blob.is_some());
    let link = find(&out, "link.txt");
    assert_eq!(link.completeness, ParseCompleteness::Unparsed(UnparsedReason::Refused));
    assert!(link.blob.is_none());
    assert!(codes_of(&out).contains(&"fs.link_skipped"));
    assert!(out.records.iter().all(|r| r.blob != Some(canary_blob)));
    // A link's target type is unknown, so it may hide a directory: always incomplete.
    assert_eq!(out.content_incomplete.len(), 1);
    assert_eq!(out.content_incomplete[0].reason, ContentIncompleteReason::LinkRefused);
    assert_eq!(out.content_incomplete[0].path.as_deref(), Some("link.txt"));
}

#[test]
fn h22_dir_symlink_makes_source_content_incomplete() {
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "x.txt", b"outside");
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "keep.txt", b"k");
    fs::create_dir(root.path().join("sub")).unwrap();
    symlink(outside.path(), root.path().join("sub/dirlink")).unwrap();

    let out = walk_root(root.path(), &WalkLimits::default());
    assert_eq!(out.records.len(), 2);
    assert_eq!(
        find(&out, "sub/dirlink").completeness,
        ParseCompleteness::Unparsed(UnparsedReason::Refused)
    );
    assert_eq!(out.content_incomplete.len(), 1);
    let inc = &out.content_incomplete[0];
    assert_eq!(inc.reason, ContentIncompleteReason::LinkRefused);
    assert_eq!(inc.path.as_deref(), Some("sub/dirlink"));
    let c = out.completeness(&ModKey("m".into()));
    assert_eq!(c.content_incomplete.len(), 1);
    assert_eq!(c.files_total, 2);
}

#[test]
fn h22_fifo_never_opened() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "a.txt", b"a");
    // `rustix::fs::mknodat` is not available on macOS, so use the `mkfifo` utility, which
    // exists on every Unix this test compiles for.
    let made = std::process::Command::new("mkfifo").arg(root.path().join("pipe")).status();
    assert!(made.unwrap().success(), "mkfifo failed");
    let path = root.path().to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || tx.send(walk_root(&path, &WalkLimits::default())));
    let out = rx.recv_timeout(Duration::from_secs(5)).expect("walk blocked on FIFO");
    let pipe = find(&out, "pipe");
    assert_eq!(pipe.completeness, ParseCompleteness::Unparsed(UnparsedReason::Refused));
    assert!(codes_of(&out).contains(&"fs.special_skipped"));
    assert!(find(&out, "a.txt").blob.is_some());
}

// macOS (APFS) and Windows are case- and normalization-insensitive, so `A.txt`/`a.txt` and
// NFC/NFD names would be one file there; the collision cannot be staged off Linux.
#[test]
#[cfg(target_os = "linux")]
fn h22_case_and_nfc_collisions_kept() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "A.txt", b"upper");
    put(root.path(), "a.txt", b"lower");
    put(root.path(), "e\u{301}.txt", b"nfd");
    put(root.path(), "\u{e9}.txt", b"nfc");
    put(root.path(), "solo.txt", b"solo");
    let out = walk_root(root.path(), &WalkLimits::default());
    assert_eq!(out.records.len(), 5);
    for key in ["a.txt", "\u{e9}.txt"] {
        let group: Vec<_> = out.records.iter().filter(|r| r.path.as_str() == key).collect();
        assert_eq!(group.len(), 2, "{key}");
        assert!(group.iter().all(|r| r.ambiguous));
        assert!(group.iter().all(|r| r.blob.is_some()));
    }
    assert!(!find(&out, "solo.txt").ambiguous);
    let n = codes_of(&out).iter().filter(|c| **c == "fs.identity_collision").count();
    assert_eq!(n, 2);
    // raw_name keeps case and is NFC.
    let raw: Vec<_> = out.records.iter().map(|r| &*r.raw_name).collect();
    assert!(raw.contains(&"A.txt") && raw.contains(&"\u{e9}.txt"));
    assert!(!raw.contains(&"e\u{301}.txt"));
}

// APFS refuses file names that are not valid UTF-8, so the name cannot be created on macOS.
#[test]
#[cfg(target_os = "linux")]
fn h22_invalid_unicode_name() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let root = tempfile::tempdir().unwrap();
    let name = OsStr::from_bytes(b"bad\xff.txt");
    fs::write(root.path().join(name), b"data").unwrap();
    let out = walk_root(root.path(), &WalkLimits::default());
    assert_eq!(out.records.len(), 1);
    let r = &out.records[0];
    assert!(r.unrepresentable);
    assert_eq!(r.path.as_str(), "bad\u{fffd}.txt");
    assert_eq!(r.os_name(), name);
    assert!(r.blob.is_some());
    assert!(codes_of(&out).contains(&"fs.unrepresentable_name"));
    let bytes = ReadOnlyRoot::open(root.path()).unwrap().read_file(&r.os_path, 1024).unwrap();
    assert_eq!(bytes, b"data");
}

#[test]
fn root_that_is_a_link_is_canonicalised() {
    let real = tempfile::tempdir().unwrap();
    put(real.path(), "x/y.txt", b"y");
    let holder = tempfile::tempdir().unwrap();
    let link = holder.path().join("mod");
    symlink(real.path(), &link).unwrap();
    let root = ReadOnlyRoot::open(&link).unwrap();
    assert_eq!(root.path(), fs::canonicalize(real.path()).unwrap());
    let out = root.walk(&WalkLimits::default());
    assert_eq!(out.records.len(), 1);
    assert_eq!(out.records[0].path.as_str(), "x/y.txt");
    assert!(out.content_incomplete.is_empty());
}

#[test]
fn changed_during_scan_detected() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "grow.txt", b"before");
    put(root.path(), "stable.txt", b"same");
    let base = root.path().to_path_buf();
    BEFORE_READ.with(|h| {
        *h.borrow_mut() = Some(Box::new(move |rel: &str| {
            if rel == "grow.txt" {
                let mut f = fs::OpenOptions::new().append(true).open(base.join(rel)).unwrap();
                f.write_all(b" and after").unwrap();
            }
        }));
    });
    let out = walk_root(root.path(), &WalkLimits::default());
    BEFORE_READ.with(|h| *h.borrow_mut() = None);
    let g = find(&out, "grow.txt");
    assert_eq!(g.completeness, ParseCompleteness::Unparsed(UnparsedReason::ChangedDuringScan));
    assert!(g.blob.is_none());
    assert!(codes_of(&out).contains(&"fs.changed_during_scan"));
    assert!(find(&out, "stable.txt").blob.is_some());
}

#[test]
fn walk_is_deterministic() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let files = ["z/1.txt", "a/b/2.txt", "M.txt", "a/C/2.txt", "common/x.txt"];
    for f in files {
        put(a.path(), f, f.as_bytes());
    }
    for f in files.iter().rev() {
        put(b.path(), f, f.as_bytes());
    }
    let (oa, ob) =
        (walk_root(a.path(), &WalkLimits::default()), walk_root(b.path(), &WalkLimits::default()));
    assert_eq!(oa, ob);
    let keys: Vec<_> = oa.records.iter().map(|r| r.path.as_bytes().to_vec()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn read_file_refuses_a_swapped_in_link() {
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "s.txt", b"secret");
    let root = tempfile::tempdir().unwrap();
    symlink(outside.path().join("s.txt"), root.path().join("l.txt")).unwrap();
    let r = ReadOnlyRoot::open(root.path()).unwrap();
    assert!(r.read_file(&["l.txt".into()], 1024).is_err());
}

#[test]
fn read_file_detects_change_since_walk() {
    let root = tempfile::tempdir().unwrap();
    put(root.path(), "f.txt", b"abc");
    let r = ReadOnlyRoot::open(root.path()).unwrap();
    assert_eq!(r.read_file(&["f.txt".into()], 3).unwrap(), b"abc");
    assert!(r.read_file(&["f.txt".into()], 2).is_err());
}
