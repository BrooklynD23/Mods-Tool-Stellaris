//! The 100-byte SQLite header, read directly: no SQLite, no locks.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

/// What the header says. Trusted for nothing but choosing a path.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) struct Header {
    /// Bytes 18 and 19 are both `2`: the database is in WAL mode.
    pub wal: bool,
    /// Bytes 40..44, the schema cookie: changes whenever the schema does.
    pub schema_cookie: u32,
    /// Bytes 60..64, `PRAGMA user_version`: what an application keeps its own schema
    /// version in. The closest thing to a launcher build identifier the file carries.
    pub user_version: u32,
    /// Bytes 68..72, `PRAGMA application_id`.
    pub application_id: u32,
}

impl Header {
    /// The identifiers to quote when a snapshot cannot be verified.
    pub(super) fn describe(&self) -> String {
        format!(
            "user_version {}, schema cookie {}, application_id {}",
            self.user_version, self.schema_cookie, self.application_id
        )
    }
}

/// Why the header could not be used.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum HeaderError {
    /// No such file: the ordinary case.
    Missing,
    /// Unreadable, too short, or not a SQLite database.
    Unusable(String),
}

const MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Read and check the header of `path`.
pub(super) fn read(path: &Path) -> Result<Header, HeaderError> {
    let mut file = File::open(path).map_err(|e| match e.kind() {
        ErrorKind::NotFound => HeaderError::Missing,
        _ => HeaderError::Unusable(format!("open: {e}")),
    })?;
    let mut buf = [0u8; 100];
    file.read_exact(&mut buf).map_err(|e| HeaderError::Unusable(format!("short header: {e}")))?;
    parse(&buf)
}

/// Interpret the 100 header bytes.
pub(super) fn parse(buf: &[u8; 100]) -> Result<Header, HeaderError> {
    if &buf[..16] != MAGIC {
        return Err(HeaderError::Unusable("not a SQLite database (bad magic)".into()));
    }
    let be = |at: usize| u32::from_be_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]);
    Ok(Header {
        wal: buf[18] == 2 && buf[19] == 2,
        schema_cookie: be(40),
        user_version: be(60),
        application_id: be(68),
    })
}
