//! Enumerating one directory by name, without opening anything inside it.
//!
//! Discovery lists `<user_dir>/mod` and the workshop root before a descriptor is read.
//! `smd-fs`'s walk opens and hashes every regular file it lists, which is content
//! reading; this lists names, kinds and sizes only. Links are never followed, and every
//! entry that is skipped or bound that trips is a diagnostic plus an incompleteness,
//! never a silent drop (doc 05, trust boundary 2).

use std::ffi::OsString;
use std::path::Path;

use crate::codes::{self, IngestDiagnostic};

/// Most directory entries listed from one directory (the `smd-fs` walk default).
pub(super) const ENTRIES_MAX: usize = 250_000;

/// What an entry is, from `lstat` semantics: a link is a link, never its target.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    File,
    Dir,
}

/// One listed entry.
pub(super) struct Entry {
    pub name: OsString,
    pub kind: Kind,
    /// Size in bytes as listed; meaningful for files.
    pub len: u64,
}

impl Entry {
    pub(super) fn text(&self) -> String {
        self.name.to_string_lossy().into_owned()
    }
}

/// A directory's entries in name order, and whether the listing is the whole truth.
pub(super) struct Listing {
    pub entries: Vec<Entry>,
    pub incomplete: bool,
    /// The directory did not exist (as opposed to failing in some other way).
    pub absent: bool,
}

fn gap(diags: &mut Vec<IngestDiagnostic>, subject: &str, why: String) {
    diags.push(IngestDiagnostic::warn(codes::DISCOVERY_INCOMPLETE, subject, why));
}

/// List `dir` (named `label` in diagnostics), skipping and reporting links, special
/// files and unreadable entries.
///
/// `care` says whether a skipped name could have been a mod; a link or special file the
/// caller has no use for is not a gap.
pub(super) fn list(
    dir: &Path,
    label: &str,
    care: impl Fn(&str) -> bool,
    diags: &mut Vec<IngestDiagnostic>,
) -> Listing {
    let mut out = Listing { entries: Vec::new(), incomplete: false, absent: false };
    let reader = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            out.absent = true;
            return out;
        }
        Err(e) => {
            gap(diags, label, format!("{} cannot be listed: {e}", dir.display()));
            out.incomplete = true;
            return out;
        }
    };
    let mut seen = 0usize;
    for item in reader {
        seen += 1;
        if seen > ENTRIES_MAX {
            let why = format!("more than {ENTRIES_MAX} entries; the rest were not listed");
            gap(diags, label, why);
            out.incomplete = true;
            break;
        }
        let entry = match item {
            Ok(e) => e,
            Err(e) => {
                gap(diags, label, format!("an entry could not be read: {e}"));
                out.incomplete = true;
                continue;
            }
        };
        let name = entry.file_name();
        let shown = format!("{label}/{}", name.to_string_lossy());
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                gap(diags, &shown, format!("cannot be inspected: {e}"));
                out.incomplete = true;
                continue;
            }
        };
        let ft = meta.file_type();
        let matters = care(&name.to_string_lossy());
        if ft.is_symlink() {
            if !matters {
                continue;
            }
            gap(diags, &shown, "is a link and was not followed (fs.link_skipped)".into());
            out.incomplete = true;
        } else if ft.is_dir() {
            out.entries.push(Entry { name, kind: Kind::Dir, len: 0 });
        } else if ft.is_file() {
            out.entries.push(Entry { name, kind: Kind::File, len: meta.len() });
        } else if matters {
            gap(diags, &shown, "is not a regular file or directory (fs.special_skipped)".into());
            out.incomplete = true;
        }
    }
    out.entries.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
