//! Unix backend: per-component `openat` from the root fd with `O_NOFOLLOW`; type, size
//! and mtime from `fstat` of the opened handle.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;

use rustix::fs::{
    AtFlags, Dir as RxDir, FileType, Mode, OFlags, Stat, fstat, open, openat, statat,
};
use rustix::io::Errno;

use super::{Kind, OpenErr, OpenedDir, OpenedFile, Stamp};

/// A directory handle plus the device of the root it descends from.
pub(crate) struct Dir {
    fd: rustix::fd::OwnedFd,
    root_dev: u64,
}

fn io_err(e: Errno) -> io::Error {
    io::Error::from_raw_os_error(e.raw_os_error())
}

// `st_dev`/`st_mtime` widths differ across Unix targets; the conversions are not
// redundant everywhere. `st_dev` is `i32` on macOS, which has no `u64: From<i32>`, so
// it is cast (the value is only ever compared for equality).
#[allow(clippy::useless_conversion, clippy::unnecessary_cast)]
fn dev_of(st: &Stat) -> u64 {
    st.st_dev as u64
}

#[allow(clippy::useless_conversion)]
fn stamp_of(st: &Stat) -> Stamp {
    Stamp {
        size: u64::try_from(st.st_size).unwrap_or(0),
        mtime: (i64::from(st.st_mtime), u32::try_from(st.st_mtime_nsec).unwrap_or(0)),
    }
}

pub(super) fn open_root(path: &Path) -> io::Result<Dir> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    let fd = open(path, flags, Mode::empty()).map_err(io_err)?;
    let st = fstat(&fd).map_err(io_err)?;
    Ok(Dir { fd, root_dev: dev_of(&st) })
}

pub(super) fn list(dir: &Dir) -> io::Result<Vec<OsString>> {
    let mut out = Vec::new();
    for entry in RxDir::read_from(&dir.fd).map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        let bytes = entry.file_name().to_bytes();
        if bytes != b"." && bytes != b".." {
            out.push(OsString::from_vec(bytes.to_vec()));
        }
    }
    Ok(out)
}

pub(super) fn classify(dir: &Dir, name: &OsStr) -> io::Result<Kind> {
    let st = statat(&dir.fd, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW).map_err(io_err)?;
    Ok(match FileType::from_raw_mode(st.st_mode) {
        FileType::RegularFile => Kind::File,
        FileType::Directory => Kind::Dir,
        // The target is never stat'ed: that could trigger an automount or network I/O.
        FileType::Symlink => Kind::Link,
        _ => Kind::Special,
    })
}

fn open_err(e: Errno) -> OpenErr {
    if e == Errno::LOOP { OpenErr::Link } else { OpenErr::Io(io_err(e)) }
}

pub(super) fn open_dir(dir: &Dir, name: &OsStr) -> Result<OpenedDir, OpenErr> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let fd = openat(&dir.fd, name.as_bytes(), flags, Mode::empty()).map_err(open_err)?;
    let st = fstat(&fd).map_err(|e| OpenErr::Io(io_err(e)))?;
    let foreign = dev_of(&st) != dir.root_dev;
    Ok(OpenedDir { dir: Dir { fd, root_dev: dir.root_dev }, foreign })
}

pub(super) fn open_file(dir: &Dir, name: &OsStr) -> Result<OpenedFile, OpenErr> {
    let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let fd = openat(&dir.fd, name.as_bytes(), flags, Mode::empty()).map_err(open_err)?;
    let st = fstat(&fd).map_err(|e| OpenErr::Io(io_err(e)))?;
    Ok(OpenedFile {
        regular: FileType::from_raw_mode(st.st_mode) == FileType::RegularFile,
        foreign: dev_of(&st) != dir.root_dev,
        stamp: stamp_of(&st),
        file: File::from(fd),
    })
}

pub(super) fn restamp(file: &File) -> io::Result<Stamp> {
    fstat(file).map(|st| stamp_of(&st)).map_err(io_err)
}
