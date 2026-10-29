//! Taking the snapshot: path A (`OnlineBackup`) and path B (`StableFileCopy`).
//!
//! Both end in an in-memory database; nothing after this module touches the launcher's
//! files. Never `immutable=1` or `nolock=1`, never a copied `-shm`, never a write to the
//! user's main file or `-wal`.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::backup::{Backup, StepResult};
use rusqlite::{Connection, OpenFlags};

use super::LauncherV2Options;
use super::verify::{Fail, Kind};

/// Largest main file or `-wal` copied or snapshotted.
pub(super) const CAP: u64 = 256 * 1024 * 1024;
/// The step name that makes path A fall back to path B.
pub(super) const ONLINE_STEP: &str = "online_backup";

fn uri(path: &Path) -> String {
    uri_of(&path.to_string_lossy())
}

/// Strip the Windows verbatim prefix: `\\?\C:\x` is `C:\x`, `\\?\UNC\h\s` is `\\h\s`.
/// Left in, SQLite reads `//?/` as a URI authority of `?` ("invalid uri authority: %3F")
/// and refuses to open the file, which silently sent every verbatim path to path B.
fn without_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        text.to_owned()
    }
}

fn uri_of(raw: &str) -> String {
    let text = without_verbatim(raw).replace('\\', "/");
    let mut s = String::from("file:");
    if !text.starts_with('/') {
        s.push('/');
    }
    for b in text.bytes() {
        match b {
            b'%' | b'?' | b'#' | b' ' => s.push_str(&format!("%{b:02X}")),
            _ => s.push(char::from(b)),
        }
    }
    s
}

/// The live database, `mode=ro` with `SQLITE_OPEN_READONLY`, a 250 ms busy timeout, and no
/// other URI flags.
pub(super) fn open_live(path: &Path) -> rusqlite::Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(format!("{}?mode=ro", uri(path)), flags)?;
    conn.busy_timeout(Duration::from_millis(250))?;
    Ok(conn)
}

/// The whole copy in one `sqlite3_backup_step(-1)`: one read transaction, so every page
/// is as of one commit, WAL included.
fn to_memory(src: &Connection, step: &'static str) -> Result<Connection, Fail> {
    let fail = |d: String| Fail::new(step, Kind::Io, d);
    let mut mem = Connection::open_in_memory().map_err(|e| fail(e.to_string()))?;
    let backup = Backup::new(src, &mut mem).map_err(|e| fail(e.to_string()))?;
    match backup.step(-1) {
        Ok(StepResult::Done) => {}
        Ok(other) => return Err(fail(format!("backup step ended {other:?}"))),
        Err(e) => return Err(fail(e.to_string())),
    }
    drop(backup);
    Ok(mem)
}

/// Path A: back the live database up into memory.
pub(super) fn online(path: &Path) -> Result<Connection, Fail> {
    let live = open_live(path).map_err(|e| Fail::new(ONLINE_STEP, Kind::Io, e.to_string()))?;
    let mem = to_memory(&live, ONLINE_STEP)?;
    drop(live);
    Ok(mem)
}

/// The two files of one pass, as hashes of the bytes actually copied.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Pass {
    main: [u8; 32],
    wal: Option<[u8; 32]>,
}

/// Copy `src` to `dst`, hashing what was written, refusing more than [`CAP`] bytes.
fn copy_hashed(src: &Path, dst: &Path) -> io::Result<[u8; 32]> {
    let mut from = File::open(src)?.take(CAP + 1);
    let mut to = File::create(dst)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > CAP {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "file is over the 256 MiB cap"));
        }
        hasher.update(&buf[..n]);
        to.write_all(&buf[..n])?;
    }
    to.flush()?;
    Ok(*hasher.finalize().as_bytes())
}

/// The `-wal` beside `main`.
pub(super) fn wal_of(main: &Path) -> PathBuf {
    let mut s = main.as_os_str().to_owned();
    s.push("-wal");
    PathBuf::from(s)
}

/// One pass: the main file and `-wal` (if present). Never `-shm`.
fn copy_pass(main: &Path, dir: &Path) -> io::Result<Pass> {
    fs::create_dir_all(dir)?;
    let name = main.file_name().unwrap_or(std::ffi::OsStr::new("launcher.sqlite"));
    let main_hash = copy_hashed(main, &dir.join(name))?;
    let wal = match copy_hashed(&wal_of(main), &wal_of(&dir.join(name))) {
        Ok(h) => Some(h),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    Ok(Pass { main: main_hash, wal })
}

/// Path B: two passes into a private `0700` directory, accepted only if byte-identical;
/// up to `attempts` times with a backoff. The directory is deleted on every exit path
/// (`TempDir` removes it on drop); its path is left in `seen` for tests.
pub(super) fn stable_copy(
    main: &Path,
    opts: &LauncherV2Options,
    seen: &mut Option<PathBuf>,
) -> Result<Connection, Fail> {
    let io_fail = |e: io::Error| Fail::new("stable_copy", Kind::Io, e.to_string());
    let tmp = tempfile::Builder::new().prefix("smd-launcher-").tempdir().map_err(io_fail)?;
    *seen = Some(tmp.path().to_path_buf());
    let name = main.file_name().unwrap_or(std::ffi::OsStr::new("launcher.sqlite"));
    for attempt in 1..=opts.attempts {
        // A fresh directory per attempt and pass: a `-wal` copied on an earlier attempt
        // must never survive into a later one in which the launcher has since
        // checkpointed and removed it, or recovery would replay stale frames over newer
        // pages and the read would be accepted as exact.
        let (d1, d2) = (format!("p1-{attempt}"), format!("p2-{attempt}"));
        let first = copy_pass(main, &tmp.path().join(d1)).map_err(io_fail)?;
        if let Some(hook) = &opts.hooks.between_passes {
            hook(attempt);
        }
        let second = copy_pass(main, &tmp.path().join(&d2)).map_err(io_fail)?;
        if first == second {
            return open_copy(&tmp.path().join(&d2).join(name));
        }
        if attempt < opts.attempts {
            std::thread::sleep(opts.backoff);
            if let Some(hook) = &opts.hooks.between_attempts {
                hook(attempt + 1);
            }
        }
    }
    let why = format!("the files changed between the two passes on all {} attempts", opts.attempts);
    Err(Fail::new("double_pass", Kind::Io, why))
}

/// Open our private copy read-write (it is ours) so SQLite runs WAL recovery against it
/// and rebuilds the index; a torn trailing frame is discarded as uncommitted.
fn open_copy(path: &Path) -> Result<Connection, Fail> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|e| Fail::new("open_copy", Kind::Io, e.to_string()))?;
    let mem = to_memory(&conn, "backup")?;
    drop(conn);
    Ok(mem)
}

#[cfg(test)]
mod tests {
    use super::uri_of;

    #[test]
    fn a_windows_verbatim_path_has_no_question_mark_authority() {
        assert_eq!(uri_of(r"\\?\C:\Users\a b\x.sqlite"), "file:/C:/Users/a%20b/x.sqlite");
        assert_eq!(uri_of(r"C:\Users\x.sqlite"), "file:/C:/Users/x.sqlite");
        assert_eq!(uri_of(r"\\?\UNC\h\s\x.sqlite"), "file://h/s/x.sqlite");
        assert_eq!(uri_of("/home/a#b/x.sqlite"), "file:/home/a%23b/x.sqlite");
    }
}
