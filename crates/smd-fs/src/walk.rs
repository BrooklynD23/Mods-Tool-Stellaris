//! The sequential, link-refusing walk (doc 05, "Filesystem").
//!
//! Entries of each directory are visited in raw-name byte order, so truncation by a tree
//! bound lands on the same entry every run; the final record list is then sorted by
//! [`NormPath`] bytes.

use std::ffi::OsString;

use smd_core::{ContentIncompleteReason, DiagSeverity, ParseCompleteness, UnparsedReason};

use crate::limits::WalkLimits;
use crate::normpath::{NormPath, shipped_name};
use crate::record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
use crate::sys::{Dir, Entry, Kind, Listed, OpenErr};

mod collide;
mod lazy;
mod read;
#[cfg(test)]
use read::BEFORE_READ;

pub(crate) use lazy::{hash_pending, open_under};

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

    /// Add a record for `comps`; it has no blob until [`hash_pending`] reads it. A path
    /// with no representable key is dropped loudly and marks the source content-incomplete.
    fn record(&mut self, comps: &[OsString], size: u64, c: ParseCompleteness) {
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
            size,
            blob: None,
            completeness: c,
            ambiguous: false,
            unrepresentable: lossy,
        });
    }

    fn unparsed(&mut self, comps: &[OsString], why: UnparsedReason, code: &'static str, d: &str) {
        self.diag(code, &rel_display(comps), d);
        self.record(comps, 0, ParseCompleteness::Unparsed(why));
    }

    fn truncate(&mut self, comps: &[OsString], detail: &str) {
        let rel = rel_display(comps);
        self.diag(codes::LIMIT_EXCEEDED, &rel, detail);
        self.incomplete(ContentIncompleteReason::Truncated, &rel);
    }

    fn dir(&mut self, dir: &Dir, comps: &mut Vec<OsString>, depth: usize) {
        let mut listed = match dir.list() {
            Ok(n) => n,
            Err(e) => return self.dir_unreadable(comps, &e),
        };
        listed.sort_by(|a, b| a.name.cmp(&b.name));
        for item in listed {
            if self.stopped {
                return;
            }
            comps.push(item.name.clone());
            self.entry(dir, comps, depth, item);
            comps.pop();
        }
    }

    fn dir_unreadable(&mut self, comps: &[OsString], e: &std::io::Error) {
        let rel = rel_display(comps);
        self.diag(codes::UNREADABLE, &rel, &format!("directory not read: {e}"));
        self.incomplete(ContentIncompleteReason::Truncated, &rel);
    }

    fn entry(&mut self, dir: &Dir, comps: &mut Vec<OsString>, depth: usize, item: Listed) {
        self.count += 1;
        if self.count > self.limits.entries {
            self.stopped = true;
            return self.truncate(comps, "entry count bound exceeded");
        }
        if rel_len(comps) > self.limits.rel_path {
            return self.truncate(comps, "relative path length bound exceeded");
        }
        let found = match item.pre {
            Some(r) => r,
            None => dir.classify(&item.name),
        };
        match found {
            Err(e) => {
                let why = UnparsedReason::Unreadable;
                self.unparsed(comps, why, codes::UNREADABLE, &format!("not classified: {e}"));
            }
            Ok(Entry { kind: Kind::File, size }) => {
                self.record(comps, size, ParseCompleteness::NotApplicable);
            }
            Ok(Entry { kind: Kind::Dir, .. }) => self.subdir(dir, comps, depth),
            Ok(Entry { kind: Kind::Link, .. }) => self.link(comps),
            Ok(Entry { kind: Kind::Special, .. }) => {
                self.refused(comps, codes::SPECIAL_SKIPPED, "not a regular file");
            }
            Ok(Entry { kind: Kind::Offline, .. }) => {
                // May be a directory: its subtree is then unseen, so the source is incomplete.
                self.refused(comps, codes::CLOUD_PLACEHOLDER, "offline cloud placeholder not read");
                self.incomplete(ContentIncompleteReason::Truncated, &rel_display(comps));
            }
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
            Err(OpenErr::Link | OpenErr::Offline) => self.link(comps),
            Err(OpenErr::Io(e)) => self.dir_unreadable(comps, &e),
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

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_boundary;
#[cfg(test)]
mod tests_lazy;
