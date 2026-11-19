//! Non-Unix backend (Windows): std only. Entries are classified by
//! [`crate::winattrs::kind_of`] (no-follow metadata, std's name-surrogate test) before
//! being opened, and re-checked on the handle. Compiled on Windows; the walk tests that
//! do not need symlinks, FIFOs or Unix permissions run on it too (unverified until a
//! Windows runner executes them).
//!
//! Each opened directory and file is checked to resolve under the root (canonicalised
//! final path; a miss sets `foreign`, which the walk reports as `fs.escaped_root`).
//!
//! ponytail: the final-path check canonicalises the *path* again rather than asking the
//! open handle (`GetFinalPathNameByHandleW` needs `unsafe`, which the crate forbids), so a
//! swap between the open and the check is still a race. Closing it needs a reviewed
//! `windows-sys` exception; until then the TOCTOU window between components stays open.

use std::ffi::OsStr;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use super::{Entry, Kind, Listed, OpenErr, OpenedDir, OpenedFile, Stamp};
use crate::winattrs::{Facts, kind_of};

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

/// A directory, by path, plus the canonical root it descends from. TOCTOU between
/// components is not closed on this backend.
pub(crate) struct Dir {
    path: PathBuf,
    root: Arc<Path>,
}

/// The final path of `p` is not under `root`: a reparse point the attribute check missed,
/// or a swap since the listing. An unresolvable path counts as outside.
fn escapes(root: &Path, p: &Path) -> bool {
    !std::fs::canonicalize(p).is_ok_and(|real| real.starts_with(root))
}

#[cfg(windows)]
fn attrs(m: &Metadata) -> u32 {
    std::os::windows::fs::MetadataExt::file_attributes(m)
}

#[cfg(not(windows))]
fn attrs(m: &Metadata) -> u32 {
    if m.is_dir() { FILE_ATTRIBUTE_DIRECTORY } else { 0 }
}

fn facts(m: &Metadata) -> Facts {
    Facts { attrs: attrs(m), surrogate: m.file_type().is_symlink() }
}

fn entry_of(m: &Metadata) -> Entry {
    Entry { kind: kind_of(facts(m)), size: m.len() }
}

fn stamp_of(m: &Metadata) -> Stamp {
    let mtime = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or((0, 0), |d| (i64::try_from(d.as_secs()).unwrap_or(0), d.subsec_nanos()));
    Stamp { size: m.len(), mtime }
}

pub(super) fn open_root(path: &Path) -> io::Result<Dir> {
    if std::fs::metadata(path)?.is_dir() {
        Ok(Dir { path: path.to_path_buf(), root: Arc::from(path) })
    } else {
        Err(io::Error::new(io::ErrorKind::NotADirectory, "root is not a directory"))
    }
}

/// `DirEntry::metadata` on Windows comes from the directory listing itself (no extra
/// open) and does not follow links, so each entry's kind and size ride along.
pub(super) fn list(dir: &Dir) -> io::Result<Vec<Listed>> {
    std::fs::read_dir(&dir.path)?
        .map(|e| {
            e.map(|e| Listed { name: e.file_name(), pre: Some(e.metadata().map(|m| entry_of(&m))) })
        })
        .collect()
}

pub(super) fn classify(dir: &Dir, name: &OsStr) -> io::Result<Entry> {
    std::fs::symlink_metadata(dir.path.join(name)).map(|m| entry_of(&m))
}

pub(super) fn open_dir(dir: &Dir, name: &OsStr) -> Result<OpenedDir, OpenErr> {
    let p = dir.path.join(name);
    let m = std::fs::symlink_metadata(&p).map_err(OpenErr::Io)?;
    match kind_of(facts(&m)) {
        Kind::Link => Err(OpenErr::Link),
        Kind::Offline => Err(OpenErr::Offline),
        Kind::Dir => {
            let foreign = escapes(&dir.root, &p);
            Ok(OpenedDir { dir: Dir { path: p, root: Arc::clone(&dir.root) }, foreign })
        }
        _ => Err(OpenErr::Io(io::Error::new(io::ErrorKind::NotADirectory, "not a directory"))),
    }
}

pub(super) fn open_file(dir: &Dir, name: &OsStr) -> Result<OpenedFile, OpenErr> {
    let p = dir.path.join(name);
    // Classify first without following; links are refused outright. Every other entry,
    // reparse point or not (dedup, WOF/CompactOS, hydrated cloud file), is opened WITHOUT
    // `FILE_FLAG_OPEN_REPARSE_POINT` so its filter yields the real content, not the stub.
    let pre = std::fs::symlink_metadata(&p).map_err(OpenErr::Io)?;
    match kind_of(facts(&pre)) {
        Kind::Link => return Err(OpenErr::Link),
        Kind::Offline => return Err(OpenErr::Offline),
        _ => {}
    }
    let mut opts = OpenOptions::new();
    opts.read(true);
    let file = opts.open(&p).map_err(OpenErr::Io)?;
    // Re-verify on the handle. A swap between the classify and the open is the residual
    // race H22 already leaves open on this path-based backend.
    let m = file.metadata().map_err(OpenErr::Io)?;
    match kind_of(facts(&m)) {
        Kind::Link => return Err(OpenErr::Link),
        Kind::Offline => return Err(OpenErr::Offline),
        _ => {}
    }
    Ok(OpenedFile {
        regular: m.is_file() && attrs(&m) & FILE_ATTRIBUTE_DIRECTORY == 0,
        foreign: escapes(&dir.root, &p),
        stamp: stamp_of(&m),
        file,
    })
}

pub(super) fn restamp(file: &File) -> io::Result<Stamp> {
    file.metadata().map(|m| stamp_of(&m))
}
