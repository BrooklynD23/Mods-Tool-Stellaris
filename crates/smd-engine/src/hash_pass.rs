//! The second walk pass: hash only the files that more than one source ships.
//!
//! A content hash is consumed for one thing, telling whether two copies of one path are
//! identical, so a path shipped by a single source is never read. That also means an
//! oversize or unreadable file at such a path is never compared and does not make its
//! source content-incomplete; at a contended path it still does.

use std::collections::HashSet;

use smd_fs::{ReadOnlyRoot, WalkLimits, WalkOutcome, WalkRecord};

/// One walked source: its open root and what the stat-only walk found.
pub(crate) type Walking = (ReadOnlyRoot, WalkOutcome);

/// Paths that two or more sources ship, `benign` ones excluded (they are not contributions).
fn contended(parts: &[Walking], benign: &dyn Fn(&str) -> bool) -> HashSet<String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut out = HashSet::new();
    for (_, outcome) in parts {
        // Records are sorted by path, so a source's own duplicates sit side by side.
        let mut prev = None;
        for r in &outcome.records {
            let p = r.path.as_str();
            if prev == Some(p) || benign(p) {
                continue;
            }
            prev = Some(p);
            if !seen.insert(p) {
                out.insert(p.to_owned());
            }
        }
    }
    out
}

/// Hash, in each source, the records at contended paths. `None` if `cancelled` fired;
/// otherwise how many files were tried (a test and benchmark handle).
pub(crate) fn hash_contended(
    parts: &mut [Walking],
    limits: &WalkLimits,
    benign: &dyn Fn(&str) -> bool,
    cancelled: &dyn Fn() -> bool,
) -> Option<usize> {
    let shared = contended(parts, benign);
    let want = |r: &WalkRecord| shared.contains(r.path.as_str());
    let mut tried = 0;
    for (root, outcome) in parts {
        if cancelled() {
            return None;
        }
        tried += root.hash_records(outcome, limits, &want);
    }
    Some(tried)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use smd_core::{ParseCompleteness, UnparsedReason};

    use super::*;

    fn put(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn walked(dir: &Path, lim: &WalkLimits) -> Walking {
        let r = ReadOnlyRoot::open(dir).unwrap();
        let o = r.walk(lim);
        (r, o)
    }

    #[test]
    fn only_contended_files_are_hashed_and_only_they_can_go_oversize() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        for d in [a.path(), b.path()] {
            put(d, "shared/big.bin", "0123456789");
            put(d, "shared/small.txt", "s");
            put(d, "descriptor.mod", "d");
        }
        put(a.path(), "only_a/big.bin", "0123456789");
        put(b.path(), "only_b.txt", "b");
        let lim = WalkLimits { file_cap: 4, ..WalkLimits::default() };
        let mut parts = vec![walked(a.path(), &lim), walked(b.path(), &lim)];
        let benign = |p: &str| p == "descriptor.mod";
        let n = hash_contended(&mut parts, &lim, &benign, &|| false);
        assert_eq!(n, Some(4), "two contended paths in each of two sources");
        let find = |i: usize, k: &str| parts[i].1.records.iter().find(|r| r.path.as_str() == k);
        let big = find(0, "shared/big.bin").unwrap();
        assert_eq!(big.completeness, ParseCompleteness::Unparsed(UnparsedReason::Oversize));
        assert!(find(0, "shared/small.txt").unwrap().blob.is_some());
        let lone = find(0, "only_a/big.bin").unwrap();
        assert_eq!(lone.completeness, ParseCompleteness::NotApplicable);
        assert!(find(0, "descriptor.mod").unwrap().blob.is_none());
        assert!(find(1, "only_b.txt").unwrap().blob.is_none());
    }

    #[test]
    fn a_cancelled_pass_reports_none() {
        let a = tempfile::tempdir().unwrap();
        put(a.path(), "x.txt", "x");
        let lim = WalkLimits::default();
        let mut parts = vec![walked(a.path(), &lim)];
        assert_eq!(hash_contended(&mut parts, &lim, &|_| false, &|| true), None);
    }
}
