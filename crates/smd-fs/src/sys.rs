//! Platform seam: everything that touches the OS lives behind these types, so the walk
//! logic is shared and the unsafe-free, link-refusing opens are written once per OS.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::Path;

#[cfg(unix)]
#[path = "sys_unix.rs"]
mod imp;
#[cfg(not(unix))]
#[path = "sys_other.rs"]
mod imp;

/// What a directory entry is, judged without following it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    File,
    Dir,
    /// A link (symlink, junction, reparse point). The target is never inspected, so it
    /// is treated as possibly a directory.
    Link,
    /// FIFO, socket, device: never opened.
    Special,
}

/// Size and mtime of an opened handle; compared before and after a read.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Stamp {
    pub size: u64,
    pub mtime: (i64, u32),
}

/// An opened file and the facts read from its handle.
pub(crate) struct OpenedFile {
    pub file: File,
    pub stamp: Stamp,
    /// The handle is a regular file.
    pub regular: bool,
    /// The handle lives on a different filesystem than the root.
    pub foreign: bool,
}

/// An opened directory handle.
pub(crate) struct OpenedDir {
    pub dir: Dir,
    /// The handle lives on a different filesystem than the root (a mount point).
    pub foreign: bool,
}

/// Why an open did not produce a handle.
#[derive(Debug)]
pub(crate) enum OpenErr {
    /// The entry turned out to be a link at open time.
    Link,
    Io(io::Error),
}

pub(crate) use imp::Dir;

impl Dir {
    /// Open `path` as the root. `path` is already canonical.
    pub(crate) fn open_root(path: &Path) -> io::Result<Dir> {
        imp::open_root(path)
    }

    /// Entry names, unsorted, without `.` and `..`.
    pub(crate) fn list(&self) -> io::Result<Vec<OsString>> {
        imp::list(self)
    }

    /// Classify an entry without following it.
    pub(crate) fn classify(&self, name: &OsStr) -> io::Result<Kind> {
        imp::classify(self, name)
    }

    /// Open a child directory without following links.
    pub(crate) fn open_dir(&self, name: &OsStr) -> Result<OpenedDir, OpenErr> {
        imp::open_dir(self, name)
    }

    /// Open a child file for reading without following links or blocking.
    pub(crate) fn open_file(&self, name: &OsStr) -> Result<OpenedFile, OpenErr> {
        imp::open_file(self, name)
    }
}

/// Re-read size and mtime from the open handle.
pub(crate) fn restamp(file: &File) -> io::Result<Stamp> {
    imp::restamp(file)
}
