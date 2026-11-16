//! Boundary cases: links with unreachable targets, loops, unreadable directories.

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::Path;

use smd_core::{ContentIncompleteReason, ParseCompleteness, UnparsedReason};

use crate::{ReadOnlyRoot, WalkLimits, WalkOutcome};

fn walk_with(root: &Path, limits: &WalkLimits) -> WalkOutcome {
    ReadOnlyRoot::open(root).unwrap().walk(limits)
}

fn walk_root(root: &Path) -> WalkOutcome {
    walk_with(root, &WalkLimits::default())
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn codes_of(o: &WalkOutcome) -> Vec<&'static str> {
    o.diagnostics.iter().map(|d| d.code).collect()
}

fn find<'a>(o: &'a WalkOutcome, key: &str) -> &'a crate::WalkRecord {
    o.records.iter().find(|r| r.path.as_str() == key).unwrap()
}

fn assert_refused_link(out: &WalkOutcome, rel: &str) {
    let r = out.records.iter().find(|r| r.path.as_str() == rel).expect("link record");
    assert_eq!(r.completeness, ParseCompleteness::Unparsed(UnparsedReason::Refused));
    assert!(out.diagnostics.iter().any(|d| d.code == "fs.link_skipped" && &*d.path == rel));
    assert!(out.content_incomplete.iter().any(|i| i.reason
        == ContentIncompleteReason::LinkRefused
        && i.path.as_deref() == Some(rel)));
}

#[test]
fn dangling_link_still_content_incomplete() {
    let root = tempfile::tempdir().unwrap();
    symlink("/nonexistent/smd-target", root.path().join("gone")).unwrap();
    assert_refused_link(&walk_root(root.path()), "gone");
}

#[test]
fn symlink_loop_still_content_incomplete() {
    let root = tempfile::tempdir().unwrap();
    symlink("b", root.path().join("a")).unwrap();
    symlink("a", root.path().join("b")).unwrap();
    let out = walk_root(root.path());
    assert_refused_link(&out, "a");
    assert_refused_link(&out, "b");
}

#[test]
fn link_into_unreachable_directory_still_content_incomplete() {
    let holder = tempfile::tempdir().unwrap();
    let locked = holder.path().join("locked");
    fs::create_dir_all(locked.join("inner")).unwrap();
    let root = tempfile::tempdir().unwrap();
    symlink(locked.join("inner"), root.path().join("l")).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    let out = walk_root(root.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    assert_refused_link(&out, "l");
}

#[test]
fn unreadable_directory_is_truncated() {
    let root = tempfile::tempdir().unwrap();
    let locked = root.path().join("locked");
    fs::create_dir(&locked).unwrap();
    fs::write(root.path().join("ok.txt"), b"x").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        // Running as root: permissions do not bind.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        return;
    }
    let out = walk_root(root.path());
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(out.diagnostics.iter().any(|d| d.code == "fs.unreadable" && &*d.path == "locked"));
    assert_eq!(out.content_incomplete.len(), 1);
    assert_eq!(out.content_incomplete[0].reason, ContentIncompleteReason::Truncated);
    assert_eq!(out.content_incomplete[0].path.as_deref(), Some("locked"));
}

#[test]
fn h22_tree_bounds() {
    let limits = WalkLimits { depth: 3, entries: 10, rel_path: 40, file_cap: 1024 };

    // Over-deep.
    let deep = tempfile::tempdir().unwrap();
    put(deep.path(), "a/b/c/d/e.txt", b"x");
    put(deep.path(), "a/ok.txt", b"x");
    let out = walk_with(deep.path(), &limits);
    assert!(codes_of(&out).contains(&"fs.limit_exceeded"));
    assert_eq!(out.content_incomplete[0].reason, ContentIncompleteReason::Truncated);
    assert_eq!(out.content_incomplete[0].path.as_deref(), Some("a/b/c/d"));
    assert!(out.records.iter().all(|r| r.path.as_str() != "a/b/c/d/e.txt"));
    assert!(out.records.iter().any(|r| r.path.as_str() == "a/ok.txt"));

    // Over-wide.
    let wide = tempfile::tempdir().unwrap();
    for i in 0..20 {
        put(wide.path(), &format!("f{i:02}.txt"), b"x");
    }
    let out = walk_with(wide.path(), &limits);
    assert_eq!(out.records.len(), 10);
    assert_eq!(out.content_incomplete.len(), 1);
    assert_eq!(out.content_incomplete[0].path.as_deref(), Some("f10.txt"));
    assert!(codes_of(&out).contains(&"fs.limit_exceeded"));

    // Over-long path.
    let long = tempfile::tempdir().unwrap();
    let name = format!("{}.txt", "n".repeat(50));
    put(long.path(), &name, b"x");
    put(long.path(), "short.txt", b"x");
    let out = walk_with(long.path(), &limits);
    assert_eq!(out.records.len(), 1);
    assert_eq!(out.content_incomplete[0].path.as_deref(), Some(name.as_str()));

    // Over the file cap: not hashed.
    let big = tempfile::tempdir().unwrap();
    put(big.path(), "big.bin", &vec![7u8; 2048]);
    let out = walk_with(big.path(), &limits);
    let r = find(&out, "big.bin");
    assert_eq!(r.completeness, ParseCompleteness::Unparsed(UnparsedReason::Oversize));
    assert!(r.blob.is_none());
    assert!(codes_of(&out).contains(&"fs.limit_exceeded"));
    assert!(out.content_incomplete.is_empty());
}
