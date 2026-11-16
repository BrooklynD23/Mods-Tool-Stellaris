//! The second pass: hash only the records the caller says will be compared.

use std::ffi::OsString;
use std::io::{Error, ErrorKind};

use smd_core::{Blob, ContentIncompleteReason, DiagSeverity, ParseCompleteness, UnparsedReason};

use super::read::hash_open;
use crate::limits::WalkLimits;
use crate::record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
use crate::sys::{Dir, OpenErr, OpenedFile};

/// The directory chain last opened: component names and their handles, same length.
/// The next file reuses the common prefix, so N files in D directories open about D
/// directories, not N times the depth. Every component was opened with the walk's link
/// and foreign checks, and a handle is the directory itself, not a path.
#[derive(Default)]
pub(crate) struct Chain {
    names: Vec<OsString>,
    dirs: Vec<Dir>,
}

#[cfg(test)]
thread_local! {
    pub(crate) static DIR_OPENS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Chain {
    fn open_parent(&mut self, root: &Dir, parents: &[OsString]) -> Result<(), OpenErr> {
        let keep = self.names.iter().zip(parents).take_while(|(a, b)| a == b).count();
        self.names.truncate(keep);
        self.dirs.truncate(keep);
        for comp in &parents[keep..] {
            #[cfg(test)]
            DIR_OPENS.with(|c| c.set(c.get() + 1));
            let opened = self.dirs.last().unwrap_or(root).open_dir(comp)?;
            if opened.foreign {
                let m = "directory is on another filesystem";
                return Err(OpenErr::Io(Error::new(ErrorKind::PermissionDenied, m)));
            }
            self.names.push(comp.clone());
            self.dirs.push(opened.dir);
        }
        Ok(())
    }

    fn open(&mut self, root: &Dir, os_path: &[OsString]) -> Result<OpenedFile, OpenErr> {
        let Some((last, parents)) = os_path.split_last() else {
            return Err(OpenErr::Io(Error::from(ErrorKind::InvalidInput)));
        };
        self.open_parent(root, parents)?;
        self.dirs.last().unwrap_or(root).open_file(last)
    }
}

/// Open `os_path` under `root` with the walk's link-refusing rules.
pub(crate) fn open_under(root: &Dir, os_path: &[OsString]) -> Result<OpenedFile, OpenErr> {
    Chain::default().open(root, os_path)
}

enum Fail {
    Link,
    Unparsed(UnparsedReason, &'static str, String),
}

fn un(why: UnparsedReason, code: &'static str, d: &str) -> Fail {
    Fail::Unparsed(why, code, d.to_owned())
}

fn rel_of(rec: &WalkRecord) -> String {
    rec.os_path.iter().map(|c| c.to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn open_checked(
    root: &Dir,
    chain: &mut Chain,
    rec: &WalkRecord,
    cap: u64,
) -> Result<OpenedFile, Fail> {
    let opened = match chain.open(root, &rec.os_path) {
        Ok(o) => o,
        Err(OpenErr::Link) => return Err(Fail::Link),
        Err(OpenErr::Offline) => {
            let m = "offline cloud placeholder not read";
            return Err(un(UnparsedReason::Refused, codes::CLOUD_PLACEHOLDER, m));
        }
        Err(OpenErr::Io(e)) => {
            let d = format!("open failed: {e}");
            return Err(Fail::Unparsed(UnparsedReason::Unreadable, codes::UNREADABLE, d));
        }
    };
    let refuse = |code, m| un(UnparsedReason::Refused, code, m);
    if !opened.regular {
        return Err(refuse(codes::SPECIAL_SKIPPED, "handle is not a regular file"));
    }
    if opened.foreign {
        return Err(refuse(codes::ESCAPED_ROOT, "handle is outside the root"));
    }
    if opened.stamp.size > cap {
        return Err(un(UnparsedReason::Oversize, codes::LIMIT_EXCEEDED, "file over the size cap"));
    }
    Ok(opened)
}

fn hash_one(
    root: &Dir,
    chain: &mut Chain,
    rec: &WalkRecord,
    limits: &WalkLimits,
) -> Result<Blob, Fail> {
    if rec.size > limits.file_cap {
        return Err(un(UnparsedReason::Oversize, codes::LIMIT_EXCEEDED, "file over the size cap"));
    }
    let opened = open_checked(root, chain, rec, limits.file_cap)?;
    hash_open(&opened, &rec.os_path).map_err(|why| match why {
        UnparsedReason::ChangedDuringScan => {
            un(why, codes::CHANGED_DURING_SCAN, "changed while reading")
        }
        _ => un(why, codes::UNREADABLE, "read failed"),
    })
}

/// Hash every regular record `want` accepts that has no blob yet. A file that cannot be
/// hashed becomes `Unparsed` with the diagnostic the eager walk used to give it. Returns
/// how many records it tried (the size-cap refusal included).
pub(crate) fn hash_pending(
    root: &Dir,
    out: &mut WalkOutcome,
    limits: &WalkLimits,
    want: &dyn Fn(&WalkRecord) -> bool,
) -> usize {
    let (mut tried, mut diags, mut incomplete) = (0, Vec::new(), Vec::new());
    // Sorted by path so neighbours share parents (see `Chain`); diagnostics are sorted below.
    let mut order: Vec<usize> = (0..out.records.len()).collect();
    order.sort_by(|a, b| out.records[*a].os_path.cmp(&out.records[*b].os_path));
    let mut chain = Chain::default();
    for i in order {
        let rec = &mut out.records[i];
        let pending = rec.blob.is_none() && rec.completeness == ParseCompleteness::NotApplicable;
        if !pending || !want(rec) {
            continue;
        }
        tried += 1;
        let rel: Box<str> = rel_of(rec).into();
        let (code, detail, why) = match hash_one(root, &mut chain, rec, limits) {
            Ok(blob) => {
                rec.blob = Some(blob);
                continue;
            }
            Err(Fail::Link) => {
                let reason = ContentIncompleteReason::LinkRefused;
                incomplete.push(Incomplete { reason, path: Some(rel.clone()) });
                (codes::LINK_SKIPPED, "link not followed".to_owned(), UnparsedReason::Refused)
            }
            Err(Fail::Unparsed(why, code, d)) => (code, d, why),
        };
        rec.completeness = ParseCompleteness::Unparsed(why);
        diags.push(FsDiagnostic {
            code,
            severity: DiagSeverity::Warn,
            path: rel,
            detail: detail.into(),
        });
    }
    out.diagnostics.extend(diags);
    out.content_incomplete.extend(incomplete);
    out.diagnostics.sort_by(|a, b| (&a.path, a.code, &a.detail).cmp(&(&b.path, b.code, &b.detail)));
    tried
}
