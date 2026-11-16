//! The sequential, link-refusing walk (doc 05, "Filesystem").
//!
//! Entries of each directory are visited in raw-name byte order, so truncation by a tree
//! bound lands on the same entry every run; the final record list is then sorted by
//! [`NormPath`] bytes.

use std::ffi::OsString;

use smd_core::{Blob, ContentIncompleteReason, DiagSeverity, ParseCompleteness, UnparsedReason};

use crate::limits::WalkLimits;
use crate::normpath::{NormPath, shipped_name};
use crate::record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
use crate::sys::{Dir, Kind, OpenErr};

mod collide;
mod read;
#[cfg(test)]
#[cfg_attr(not(unix), allow(unused_imports))] // the race tests that use it are unix-only
use read::BEFORE_READ;
use read::hash_open;

pub(crate) fn walk(root: &Dir, limits: &WalkLimits) -> WalkOutcome {
    let mut w = Walker { limits: *limits, out: WalkOutcome::default(), count: 0, stopped: false };
    w.dir(root, &mut Vec::new(), 0);
    w.finish()
}

struct Walker {
    limits: WalkLimits,
    out: WalkOutcome,
    count: usize,
    stopped: bool,
}

fn rel_display(comps: &[OsString]) -> String {
    let parts: Vec<_> = comps.iter().map(|c| c.to_string_lossy()).collect();
    parts.join("/")
}

fn rel_len(comps: &[OsString]) -> usize {
    comps.iter().map(|c| c.len()).sum::<usize>() + comps.len().saturating_sub(1)
}

/// Identity key for a relative path, and whether any component needed a lossy key.
fn key_of(comps: &[OsString]) -> (Option<NormPath>, bool) {
    let mut lossy = false;
    let parts: Vec<String> = comps
        .iter()
        .map(|c| {
            if c.to_str().is_none_or(|s| s.contains('\\')) {
                lossy = true;
            }
            c.to_string_lossy().replace('\\', "\u{FFFD}")
        })
        .collect();
    (NormPath::new(&parts.join("/")).ok(), lossy)
}

impl Walker {
    fn diag(&mut self, code: &'static str, rel: &str, detail: &str) {
        self.out.diagnostics.push(FsDiagnostic {
            code,
            severity: DiagSeverity::Warn,
            path: rel.into(),
            detail: detail.into(),
        });
    }

    /// An empty `rel` (the root itself) is recorded as no path.
    fn incomplete(&mut self, reason: ContentIncompleteReason, rel: &str) {
        let path = (!rel.is_empty()).then(|| rel.into());
        self.out.content_incomplete.push(Incomplete { reason, path });
    }

    /// Add a record for `comps`. A path with no representable key is dropped loudly and
    /// marks the source content-incomplete.
    fn record(&mut self, comps: &[OsString], blob: Option<Blob>, c: ParseCompleteness) {
        let rel = rel_display(comps);
        let (Some(path), lossy) = key_of(comps) else {
            self.diag(codes::UNREADABLE, &rel, "path has no representable identity key");
            self.incomplete(ContentIncompleteReason::Truncated, &rel);
            return;
        };
        if lossy {
            self.diag(codes::UNREPRESENTABLE_NAME, &rel, "name is not valid Unicode or holds `\\`");
        }
        let last = comps.last().map(|c| c.to_string_lossy()).unwrap_or_default();
        self.out.records.push(WalkRecord {
            path,
            raw_name: shipped_name(&last),
            os_path: comps.to_vec(),
            blob,
            completeness: c,
            ambiguous: false,
            unrepresentable: lossy,
        });
    }

    fn unparsed(&mut self, comps: &[OsString], why: UnparsedReason, code: &'static str, d: &str) {
        self.diag(code, &rel_display(comps), d);
        self.record(comps, None, ParseCompleteness::Unparsed(why));
    }

    fn truncate(&mut self, comps: &[OsString], detail: &str) {
        let rel = rel_display(comps);
        self.diag(codes::LIMIT_EXCEEDED, &rel, detail);
        self.incomplete(ContentIncompleteReason::Truncated, &rel);
    }

    fn dir(&mut self, dir: &Dir, comps: &mut Vec<OsString>, depth: usize) {
        let mut names = match dir.list() {
            Ok(n) => n,
            Err(e) => return self.dir_unreadable(comps, &e),
        };
        names.sort();
        for name in names {
            if self.stopped {
                return;
            }
            comps.push(name);
            self.entry(dir, comps, depth);
            comps.pop();
        }
    }

    fn dir_unreadable(&mut self, comps: &[OsString], e: &std::io::Error) {
        let rel = rel_display(comps);
        self.diag(codes::UNREADABLE, &rel, &format!("directory not read: {e}"));
        self.incomplete(ContentIncompleteReason::Truncated, &rel);
    }

    fn entry(&mut self, dir: &Dir, comps: &mut Vec<OsString>, depth: usize) {
        self.count += 1;
        if self.count > self.limits.entries {
            self.stopped = true;
            return self.truncate(comps, "entry count bound exceeded");
        }
        if rel_len(comps) > self.limits.rel_path {
            return self.truncate(comps, "relative path length bound exceeded");
        }
        let Some(name) = comps.last().cloned() else { return };
        match dir.classify(&name) {
            Err(e) => {
                let why = UnparsedReason::Unreadable;
                self.unparsed(comps, why, codes::UNREADABLE, &format!("not classified: {e}"));
            }
            Ok(Kind::File) => self.file(dir, comps),
            Ok(Kind::Dir) => self.subdir(dir, comps, depth),
            Ok(Kind::Link) => self.link(comps),
            Ok(Kind::Special) => self.refused(comps, codes::SPECIAL_SKIPPED, "not a regular file"),
        }
    }

    fn refused(&mut self, comps: &[OsString], code: &'static str, detail: &str) {
        self.unparsed(comps, UnparsedReason::Refused, code, detail);
    }

    /// Every link is refused alike: its target is never inspected, so it may be a
    /// directory whose contents the source then lacks.
    fn link(&mut self, comps: &[OsString]) {
        self.refused(comps, codes::LINK_SKIPPED, "link not followed");
        self.incomplete(ContentIncompleteReason::LinkRefused, &rel_display(comps));
    }

    fn subdir(&mut self, dir: &Dir, comps: &mut Vec<OsString>, depth: usize) {
        if depth + 1 > self.limits.depth {
            return self.truncate(comps, "directory depth bound exceeded");
        }
        let Some(name) = comps.last().cloned() else { return };
        match dir.open_dir(&name) {
            Ok(o) if o.foreign => {
                let rel = rel_display(comps);
                self.diag(codes::ESCAPED_ROOT, &rel, "directory is on another filesystem");
                self.incomplete(ContentIncompleteReason::LinkRefused, &rel);
            }
            Ok(o) => self.dir(&o.dir, comps, depth + 1),
            Err(OpenErr::Link) => self.link(comps),
            Err(OpenErr::Io(e)) => self.dir_unreadable(comps, &e),
        }
    }

    fn file(&mut self, dir: &Dir, comps: &[OsString]) {
        let Some(name) = comps.last() else { return };
        let opened = match dir.open_file(name) {
            Ok(o) => o,
            Err(OpenErr::Link) => return self.link(comps),
            Err(OpenErr::Io(e)) => {
                let why = UnparsedReason::Unreadable;
                return self.unparsed(comps, why, codes::UNREADABLE, &format!("open failed: {e}"));
            }
        };
        if !opened.regular {
            return self.refused(comps, codes::SPECIAL_SKIPPED, "handle is not a regular file");
        }
        if opened.foreign {
            return self.refused(comps, codes::ESCAPED_ROOT, "handle is outside the root");
        }
        if opened.stamp.size > self.limits.file_cap {
            let why = UnparsedReason::Oversize;
            return self.unparsed(comps, why, codes::LIMIT_EXCEEDED, "file over the size cap");
        }
        match hash_open(&opened, &rel_display(comps)) {
            Ok(blob) => self.record(comps, Some(blob), ParseCompleteness::NotApplicable),
            Err(UnparsedReason::ChangedDuringScan) => {
                let why = UnparsedReason::ChangedDuringScan;
                self.unparsed(comps, why, codes::CHANGED_DURING_SCAN, "changed while reading");
            }
            Err(why) => self.unparsed(comps, why, codes::UNREADABLE, "read failed"),
        }
    }

    fn finish(mut self) -> WalkOutcome {
        self.out.records.sort_by(|a, b| (&a.path, &a.os_path).cmp(&(&b.path, &b.os_path)));
        collide::mark(&mut self.out);
        self.out
            .diagnostics
            .sort_by(|a, b| (&a.path, a.code, &a.detail).cmp(&(&b.path, b.code, &b.detail)));
        self.out
    }
}

#[cfg(all(test, unix))]
mod tests;
#[cfg(all(test, unix))]
mod tests_boundary;
