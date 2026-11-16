//! The stat-only walk and the contended-only hash pass.

use std::cell::Cell;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::rc::Rc;

use smd_core::{ContentIncompleteReason, ParseCompleteness, UnparsedReason};

use super::BEFORE_READ;
use crate::{ReadOnlyRoot, WalkLimits, WalkOutcome};

const CAP: WalkLimits = WalkLimits { depth: 8, entries: 100, rel_path: 100, file_cap: 8 };

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

fn only(rel: &'static str) -> impl Fn(&crate::WalkRecord) -> bool {
    move |r| r.path.as_str() == rel
}

fn unparsed(o: &WalkOutcome) -> usize {
    o.records.iter().filter(|r| matches!(r.completeness, ParseCompleteness::Unparsed(_))).count()
}

#[test]
fn walk_alone_opens_nothing_and_flags_nothing() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "big.bin", &[1u8; 64]);
    put(d.path(), "small.txt", b"x");
    let reads = Rc::new(Cell::new(0));
    let r2 = Rc::clone(&reads);
    BEFORE_READ.with(|h| *h.borrow_mut() = Some(Box::new(move |_| r2.set(r2.get() + 1))));
    let out = ReadOnlyRoot::open(d.path()).unwrap().walk(&CAP);
    BEFORE_READ.with(|h| *h.borrow_mut() = None);
    assert_eq!(reads.get(), 0);
    assert!(out.records.iter().all(|r| r.blob.is_none()));
    assert_eq!(out.records.iter().map(|r| r.size).max(), Some(64));
    assert_eq!((unparsed(&out), out.diagnostics.len(), out.content_incomplete.len()), (0, 0, 0));
}

#[test]
fn hashing_reuses_the_parent_directory_chain() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..5 {
        put(d.path(), &format!("a/b/c/f{i}.txt"), b"x");
    }
    let r = ReadOnlyRoot::open(d.path()).unwrap();
    let mut out = r.walk(&CAP);
    super::lazy::DIR_OPENS.with(|c| c.set(0));
    let tried = r.hash_records(&mut out, &CAP, &|_| true);
    assert_eq!(tried, 5);
    assert!(out.records.iter().all(|r| r.blob.is_some()));
    assert_eq!(super::lazy::DIR_OPENS.with(Cell::get), 3, "a, b, c once, not 5 x 3");
}

#[test]
fn uncontended_oversize_stays_complete() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "big.bin", &[1u8; 64]);
    put(d.path(), "shared.txt", b"x");
    let r = ReadOnlyRoot::open(d.path()).unwrap();
    let mut out = r.walk(&CAP);
    let tried = r.hash_records(&mut out, &CAP, &only("shared.txt"));
    assert_eq!(tried, 1);
    assert_eq!(unparsed(&out), 0);
    assert!(out.diagnostics.is_empty());
}

#[test]
fn contended_oversize_is_unparsed_and_reported() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "big.bin", &[1u8; 64]);
    let r = ReadOnlyRoot::open(d.path()).unwrap();
    let mut out = r.walk(&CAP);
    r.hash_records(&mut out, &CAP, &only("big.bin"));
    assert_eq!(out.records[0].completeness, ParseCompleteness::Unparsed(UnparsedReason::Oversize));
    assert!(out.records[0].blob.is_none());
    assert_eq!(out.diagnostics[0].code, "fs.limit_exceeded");
}

#[test]
fn identical_content_is_still_detected_across_roots() {
    let (a, b, c) =
        (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    put(a.path(), "x/f.txt", b"same");
    put(b.path(), "x/f.txt", b"same");
    put(c.path(), "x/f.txt", b"diff");
    let blob = |p: &Path| {
        let r = ReadOnlyRoot::open(p).unwrap();
        let mut out = r.walk(&CAP);
        r.hash_records(&mut out, &CAP, &|_| true);
        out.records[0].blob.unwrap()
    };
    assert_eq!(blob(a.path()), blob(b.path()));
    assert_ne!(blob(a.path()), blob(c.path()));
}

#[test]
fn hash_count_equals_contended_count() {
    let d = tempfile::tempdir().unwrap();
    for i in 0..10 {
        put(d.path(), &format!("f{i}.txt"), b"x");
    }
    let reads = Rc::new(Cell::new(0));
    let r2 = Rc::clone(&reads);
    BEFORE_READ.with(|h| *h.borrow_mut() = Some(Box::new(move |_| r2.set(r2.get() + 1))));
    let r = ReadOnlyRoot::open(d.path()).unwrap();
    let mut out = r.walk(&CAP);
    let tried = r.hash_records(&mut out, &CAP, &|rec| rec.path.as_str() < "f3");
    BEFORE_READ.with(|h| *h.borrow_mut() = None);
    assert_eq!((tried, reads.get()), (3, 3));
    assert_eq!(out.records.iter().filter(|r| r.blob.is_some()).count(), 3);
}

#[test]
fn a_file_swapped_for_a_link_before_hashing_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    put(outside.path(), "s.txt", b"secret");
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "f.txt", b"x");
    let r = ReadOnlyRoot::open(d.path()).unwrap();
    let mut out = r.walk(&CAP);
    fs::remove_file(d.path().join("f.txt")).unwrap();
    symlink(outside.path().join("s.txt"), d.path().join("f.txt")).unwrap();
    r.hash_records(&mut out, &CAP, &|_| true);
    assert!(out.records[0].blob.is_none());
    assert_eq!(out.records[0].completeness, ParseCompleteness::Unparsed(UnparsedReason::Refused));
    assert_eq!(out.content_incomplete[0].reason, ContentIncompleteReason::LinkRefused);
}
