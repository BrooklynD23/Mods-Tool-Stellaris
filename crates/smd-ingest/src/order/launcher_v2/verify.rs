//! Verification by read-back (doc 05): integrity, schema, data invariants, digest.

use rusqlite::Connection;

use super::rows::{self, Rows};
use super::schema::{self, Schema};

/// What kind of refusal a failed step is, which decides the diagnostic and `OrderError`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    /// `integrity_check` failed, or SQLite called the file malformed.
    Corrupt,
    /// The schema (or a value's encoding) is not the one the reader knows.
    Schema,
    /// More than one active, non-removed playset.
    MultipleActive,
    /// No active, non-removed playset.
    NoActive,
    /// Another data invariant failed: duplicate positions, a dangling `modId`.
    Invariant,
    /// An I/O or SQLite error.
    Io,
}

/// A failed verification step. A refusal, never a repair.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) struct Fail {
    pub step: &'static str,
    pub kind: Kind,
    pub detail: String,
}

impl Fail {
    pub(super) fn new(step: &'static str, kind: Kind, detail: impl Into<String>) -> Fail {
        Fail { step, kind, detail: detail.into() }
    }
}

/// A snapshot that passed every check.
pub(super) struct Verified {
    pub schema: Schema,
    pub rows: Rows,
}

/// `PRAGMA integrity_check`, not `quick_check`: the database is a few MB and the full
/// check also validates index and table agreement, which is where a torn page hides.
fn integrity(conn: &Connection) -> Result<(), Fail> {
    let corrupt = |d: String| Fail::new("integrity_check", Kind::Corrupt, d);
    let mut st = conn.prepare("PRAGMA integrity_check").map_err(|e| corrupt(e.to_string()))?;
    let lines: Vec<String> = st
        .query_map([], |r| r.get::<_, String>(0))
        .and_then(Iterator::collect)
        .map_err(|e| corrupt(e.to_string()))?;
    if lines == ["ok"] {
        Ok(())
    } else {
        Err(corrupt(lines.into_iter().take(5).collect::<Vec<_>>().join(" | ")))
    }
}

/// Run steps 1, 2, 3 and 5 on an in-memory snapshot.
pub(super) fn verify(conn: &Connection) -> Result<Verified, Fail> {
    integrity(conn)?;
    let schema = schema::probe(conn)?;
    let rows = rows::read(conn, schema)?;
    Ok(Verified { schema, rows })
}
