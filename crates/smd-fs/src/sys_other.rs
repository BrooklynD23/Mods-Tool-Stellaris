//! Non-Unix backend (Windows): std only. Entries are opened with
//! `FILE_FLAG_OPEN_REPARSE_POINT` and classified by [`crate::winattrs::kind_of`] from the
//! handle's attributes and std's name-surrogate test. Compiled on Windows but not
//! exercised by the Linux-only test run (the classifier is).
//!
//! NOT FUNCTIONALLY COMPLETE (H22 stays open for Windows): it works from paths, so the
//! TOCTOU window between components is open, and there is no final-path-under-root check
//! (`fs.escaped_root`). Do not ship a Windows build before those are implemented.

use std::ffi::OsStr;
use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::{Entry, Kind, Listed, OpenErr, OpenedDir, OpenedFile, Stamp};
use crate::winattrs::{Facts, is_offline, kind_of};

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
#[cfg(windows)]
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;

/// A directory, by path. TOCTOU between components is not closed on this backend.
pub(crate) struct Dir {
    path: PathBuf,
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
        Ok(Dir { path: path.to_path_buf() })
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
        Kind::Dir => Ok(OpenedDir { dir: Dir { path: p }, foreign: false }),
        _ => Err(OpenErr::Io(io::Error::new(io::ErrorKind::NotADirectory, "not a directory"))),
    }
}

pub(super) fn open_file(dir: &Dir, name: &OsStr) -> Result<OpenedFile, OpenErr> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    std::os::windows::fs::OpenOptionsExt::custom_flags(&mut opts, FILE_FLAG_OPEN_REPARSE_POINT);
    let file = opts.open(dir.path.join(name)).map_err(OpenErr::Io)?;
    let m = file.metadata().map_err(OpenErr::Io)?;
    if m.file_type().is_symlink() {
        return Err(OpenErr::Link);
    }
    if attrs(&m) & FILE_ATTRIBUTE_DIRECTORY == 0 && is_offline(attrs(&m)) {
        return Err(OpenErr::Offline);
    }
    Ok(OpenedFile {
        regular: m.is_file() && attrs(&m) & FILE_ATTRIBUTE_DIRECTORY == 0,
        foreign: false,
        stamp: stamp_of(&m),
        file,
    })
}

pub(super) fn restamp(file: &File) -> io::Result<Stamp> {
    file.metadata().map(|m| stamp_of(&m))
}
