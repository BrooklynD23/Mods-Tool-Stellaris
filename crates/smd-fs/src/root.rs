//! The read-only root handle: the only way `smd-fs` yields bytes (doc 14).

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::limits::WalkLimits;
use crate::record::WalkOutcome;
use crate::sys::{Dir, OpenErr, restamp};

/// A resolved, canonicalised mod or game root, open for reading.
///
/// There is no write API, and no API that takes a bare path and returns bytes: files are
/// reached by walking from this handle or by the raw OS components of a [`WalkRecord`]
/// (`crate::WalkRecord`) it produced.
pub struct ReadOnlyRoot {
    path: PathBuf,
    dir: Dir,
}

impl ReadOnlyRoot {
    /// Open `path` as a root. A root that is itself a link (a mod folder moved to another
    /// drive) is canonicalised once; only links *inside* the root are refused later.
    ///
    /// # Errors
    /// The path does not exist, is not a directory, or cannot be opened.
    pub fn open(path: &Path) -> io::Result<ReadOnlyRoot> {
        let path = std::fs::canonicalize(path)?;
        let dir = Dir::open_root(&path)?;
        Ok(ReadOnlyRoot { path, dir })
    }

    /// The canonical root path, for display.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Enumerate and hash everything under the root. See [`WalkOutcome`].
    #[must_use]
    pub fn walk(&self, limits: &WalkLimits) -> WalkOutcome {
        crate::walk::walk(&self.dir, limits)
    }

    /// Read one file named by raw OS components (a [`WalkRecord`](crate::WalkRecord)'s
    /// `os_path`), re-opening it with the same link-refusing rules as the walk, up to
    /// `cap` bytes.
    ///
    /// # Errors
    /// Any open or read error; a link, a non-regular file, or a handle on another
    /// filesystem is `PermissionDenied`; a file over `cap`, or whose length or mtime no
    /// longer matches the handle's pre-read stamp, is `InvalidData`.
    pub fn read_file(&self, os_path: &[OsString], cap: u64) -> io::Result<Vec<u8>> {
        let denied = |m: &'static str| io::Error::new(io::ErrorKind::PermissionDenied, m);
        let Some((last, parents)) = os_path.split_last() else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty path"));
        };
        let mut cur: Option<Dir> = None;
        for comp in parents {
            let parent = cur.as_ref().unwrap_or(&self.dir);
            let opened = parent.open_dir(comp).map_err(into_io)?;
            if opened.foreign {
                return Err(denied("directory is on another filesystem"));
            }
            cur = Some(opened.dir);
        }
        let opened = cur.as_ref().unwrap_or(&self.dir).open_file(last).map_err(into_io)?;
        if !opened.regular || opened.foreign {
            return Err(denied("not a regular file under the root"));
        }
        if opened.stamp.size > cap {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file exceeds cap"));
        }
        let changed = |m: &'static str| io::Error::new(io::ErrorKind::InvalidData, m);
        let mut buf = Vec::new();
        (&opened.file).take(cap.saturating_add(1)).read_to_end(&mut buf)?;
        if buf.len() as u64 > cap {
            return Err(changed("file grew past cap while reading"));
        }
        if buf.len() as u64 != opened.stamp.size || restamp(&opened.file)? != opened.stamp {
            return Err(changed("file changed during read"));
        }
        Ok(buf)
    }
}

fn into_io(e: OpenErr) -> io::Error {
    match e {
        OpenErr::Link => io::Error::new(io::ErrorKind::PermissionDenied, "link refused"),
        OpenErr::Io(e) => e,
    }
}
