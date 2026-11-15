//! The second pass: hash only the records the caller says will be compared.

use std::ffi::OsString;
use std::io::{Error, ErrorKind};

use smd_core::{Blob, ContentIncompleteReason, DiagSeverity, ParseCompleteness, UnparsedReason};

use super::read::hash_open;
use crate::limits::WalkLimits;
use crate::record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
use crate::sys::{Dir, OpenErr, OpenedFile};

/// Open `os_path` under `root` with the walk's link-refusing rules.
pub(crate) fn open_under(root: &Dir, os_path: &[OsString]) -> Result<OpenedFile, OpenErr> {
    let Some((last, parents)) = os_path.split_last() else {
        return Err(OpenErr::Io(Error::from(ErrorKind::InvalidInput)));
    };
    let mut cur: Option<Dir> = None;
    for comp in parents {
        let opened = cur.as_ref().unwrap_or(root).open_dir(comp)?;
        if opened.foreign {
            let m = "directory is on another filesystem";
            return Err(OpenErr::Io(Error::new(ErrorKind::PermissionDenied, m)));
        }
        cur = Some(opened.dir);
    }
    cur.as_ref().unwrap_or(root).open_file(last)
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

fn open_checked(root: &Dir, rec: &WalkRecord, cap: u64) -> Result<OpenedFile, Fail> {
    let opened = match open_under(root, &rec.os_path) {
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

fn hash_one(root: &Dir, rec: &WalkRecord, limits: &WalkLimits) -> Result<Blob, Fail> {
    if rec.size > limits.file_cap {
        return Err(un(UnparsedReason::Oversize, codes::LIMIT_EXCEEDED, "file over the size cap"));
    }
    let opened = open_checked(root, rec, limits.file_cap)?;
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
    for rec in &mut out.records {
        let pending = rec.blob.is_none() && rec.completeness == ParseCompleteness::NotApplicable;
        if !pending || !want(rec) {
            continue;
        }
        tried += 1;
        let rel: Box<str> = rel_of(rec).into();
        let (code, detail, why) = match hash_one(root, rec, limits) {
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
