//! `LauncherV2Sqlite`: the launcher's active playset, read from a verified snapshot.
//!
//! This is the only module in `smd-ingest` that touches SQLite (doc 05); the parse cache
//! in `smd-index` opens its own database and never the launcher's.
//!
//! The schema is Irony's `v4` model and is **assumed, not verified** against any real
//! launcher (VERIFY V03, V41). The protocol is in doc 05, "Snapshotting
//! `launcher-v2.sqlite`"; failures name the step and the path taken and the chain falls
//! through, so an unverified snapshot never yields `Exact`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rusqlite::Connection;
use smd_core::{
    Confidence, LoadOrderSource, OrderError, Playset, ResolvedLocations, SnapshotMethod,
};

use super::{ChainSource, SourceAttempt, SourceOptions};
use crate::codes::{self, IngestDiagnostic};
use crate::{Discovery, KEY_USER_DIR, discover};

mod header;
mod mount;
mod playset;
mod rows;
mod schema;
mod snapshot;
mod verify;

use verify::{Fail, Kind, Verified};

/// Registry key.
pub const ID: &str = "launcher_v2_sqlite";
/// The database file name, in the user directory.
pub const FILE: &str = "launcher-v2.sqlite";

type Hook = Arc<dyn Fn(u32) + Send + Sync>;

/// Seams that let tests change files while a read is in progress.
#[derive(Clone, Default)]
pub struct Hooks {
    /// Path B: runs between the two passes of each attempt, with the attempt number.
    pub between_passes: Option<Hook>,
    /// Path B: runs after the backoff and before the next attempt's first pass, with that
    /// attempt's number.
    pub between_attempts: Option<Hook>,
    /// Path A: runs after each snapshot and before the live digest, with the round.
    pub after_snapshot: Option<Hook>,
}

impl std::fmt::Debug for Hooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hooks")
            .field("between_passes", &self.between_passes.is_some())
            .field("between_attempts", &self.between_attempts.is_some())
            .field("after_snapshot", &self.after_snapshot.is_some())
            .finish()
    }
}

/// Knobs for the launcher reader. The defaults are the design's.
#[derive(Clone, Debug)]
pub struct LauncherV2Options {
    /// Permit path A. `false` forces path B (the copy fallback).
    pub allow_online: bool,
    /// Path B attempts.
    pub attempts: u32,
    /// Pause between failed path B attempts.
    pub backoff: Duration,
    /// Test seams.
    pub hooks: Hooks,
}

impl Default for LauncherV2Options {
    fn default() -> Self {
        LauncherV2Options {
            allow_online: true,
            attempts: 3,
            backoff: Duration::from_millis(200),
            hooks: Hooks::default(),
        }
    }
}

/// Re-snapshots after a live digest mismatch, before giving up with `launcher_changing`.
const RESNAPSHOTS: u32 = 3;

/// The launcher-database source.
#[derive(Clone, Debug, Default)]
pub struct LauncherV2Sqlite {
    opts: SourceOptions,
}

/// What a read concluded, with the observable facts tests need.
#[derive(Debug)]
pub struct LauncherRun {
    /// The attempt as the chain sees it.
    pub attempt: SourceAttempt,
    /// The private directory path B used, which must no longer exist.
    pub temp_dir: Option<PathBuf>,
}

struct Loaded {
    verified: Verified,
    method: SnapshotMethod,
}

/// A failure and the path it happened on.
struct Failed {
    fail: Fail,
    path: &'static str,
}

impl LauncherV2Sqlite {
    /// A reader with these options.
    #[must_use]
    pub fn new(opts: SourceOptions) -> LauncherV2Sqlite {
        LauncherV2Sqlite { opts }
    }

    /// Read the active playset, reporting the private directory too.
    #[must_use]
    pub fn run(&self, loc: &ResolvedLocations) -> LauncherRun {
        self.run_with(loc, &discover(loc, &self.opts.discovery))
    }

    /// As [`run`](LauncherV2Sqlite::run), over a discovery already made.
    #[must_use]
    pub fn run_with(&self, loc: &ResolvedLocations, found: &Discovery) -> LauncherRun {
        let mut temp_dir = None;
        let mut diagnostics = Vec::new();
        let mut build = String::new();
        let result = self.read(loc, &mut temp_dir, &mut build, &mut diagnostics);
        let result = match result {
            Ok(loaded) => {
                let rows = &loaded.verified.rows;
                Ok(playset::build(rows, &found.mods, loaded.method, &mut diagnostics))
            }
            Err(Some(failed)) => Err(refuse(failed, &build, &mut diagnostics)),
            Err(None) => Err(OrderError::NotPresent),
        };
        LauncherRun { attempt: SourceAttempt { result, diagnostics }, temp_dir }
    }

    /// `Err(None)` means the file is not there: the ordinary case.
    fn read(
        &self,
        loc: &ResolvedLocations,
        temp: &mut Option<PathBuf>,
        build: &mut String,
        diags: &mut Vec<IngestDiagnostic>,
    ) -> Result<Loaded, Option<Failed>> {
        let user = loc.get(KEY_USER_DIR).ok_or(None)?;
        let file = user.join(FILE);
        let none = |step, detail: String| {
            Some(Failed { fail: Fail::new(step, Kind::Io, detail), path: "none" })
        };
        let head = header::read(&file).map_err(|e| match e {
            header::HeaderError::Missing => None,
            header::HeaderError::Unusable(d) => none("header", d),
        })?;
        *build = head.describe();
        match std::fs::metadata(&file) {
            Ok(m) if m.len() > snapshot::CAP => {
                return Err(none("size_cap", "file is over 256 MiB".into()));
            }
            Err(e) => return Err(none("size_cap", e.to_string())),
            Ok(_) => {}
        }
        if self.path_a_allowed(&file, head) {
            match self.online(&file, diags) {
                Err(f) if f.step == snapshot::ONLINE_STEP => {}
                other => {
                    return other.map_err(|fail| Some(Failed { fail, path: "A (OnlineBackup)" }));
                }
            }
        }
        let verified = snapshot::stable_copy(&file, &self.opts.launcher, temp)
            .and_then(|mem| verify::verify(&mem))
            .map_err(|fail| Some(Failed { fail, path: "B (StableFileCopy)" }))?;
        Ok(Loaded { verified, method: SnapshotMethod::StableFileCopy })
    }

    fn path_a_allowed(&self, file: &Path, head: header::Header) -> bool {
        let shm = {
            let mut s = file.as_os_str().to_owned();
            s.push("-shm");
            PathBuf::from(s)
        };
        self.opts.launcher.allow_online
            && head.wal
            && snapshot::wal_of(file).exists()
            && shm.exists()
            && file.parent().is_some_and(mount::is_local)
    }

    /// Path A with live-digest comparison: a new snapshot up to [`RESNAPSHOTS`] times if
    /// the launcher committed in between or a transient I/O error (`SQLITE_BUSY` and kin)
    /// hit the verification; then path B takes over.
    fn online(&self, file: &Path, diags: &mut Vec<IngestDiagnostic>) -> Result<Loaded, Fail> {
        let mut round = 0;
        loop {
            let (verified, live) = match self.online_round(file, round) {
                Ok(x) => x,
                Err(f) if f.kind == Kind::Io && f.step != snapshot::ONLINE_STEP => {
                    if round >= RESNAPSHOTS {
                        let why = format!("{} failed on {} tries: {}", f.step, round + 1, f.detail);
                        return Err(Fail::new(snapshot::ONLINE_STEP, Kind::Io, why));
                    }
                    round += 1;
                    std::thread::sleep(self.opts.launcher.backoff);
                    continue;
                }
                Err(f) => return Err(f),
            };
            if live == verified.rows.digest {
                return Ok(Loaded { verified, method: SnapshotMethod::OnlineBackup });
            }
            if round >= RESNAPSHOTS {
                let why = "the launcher kept committing while it was read; the order is \
                           consistent but may be seconds stale. Rescan once it is idle";
                diags.push(IngestDiagnostic::warn(codes::LAUNCHER_CHANGING, FILE, why));
                return Ok(Loaded { verified, method: SnapshotMethod::OnlineBackup });
            }
            round += 1;
        }
    }

    /// One snapshot, its verification, and the live digest to compare with.
    fn online_round(&self, file: &Path, round: u32) -> Result<(Verified, [u8; 32]), Fail> {
        let mem = snapshot::online(file)?;
        if let Some(hook) = &self.opts.launcher.hooks.after_snapshot {
            hook(round);
        }
        let verified = verify::verify(&mem)?;
        let live = live_digest(file, &verified)?;
        Ok((verified, live))
    }
}

/// The logical digest of the live database, in one fresh read transaction.
fn live_digest(file: &Path, v: &Verified) -> Result<[u8; 32], Fail> {
    let fail = |e: rusqlite::Error| Fail::new("live_digest", Kind::Io, e.to_string());
    let live: Connection = snapshot::open_live(file).map_err(fail)?;
    live.execute_batch("BEGIN").map_err(fail)?;
    let read = rows::read(&live, v.schema);
    live.execute_batch("COMMIT").map_err(fail)?;
    read.map(|r| r.digest).map_err(|f| Fail::new("live_digest", f.kind, f.detail))
}

/// Turn a failure into the diagnostics and the `OrderError` the chain falls through on.
fn refuse(failed: Failed, build: &str, diags: &mut Vec<IngestDiagnostic>) -> OrderError {
    let Failed { fail, path } = failed;
    let ident =
        if build.is_empty() { String::new() } else { format!(" (launcher database: {build})") };
    let why = format!("step {} failed on path {path}: {}{ident}", fail.step, fail.detail);
    diags.push(IngestDiagnostic::error(codes::LAUNCHER_SNAPSHOT_UNVERIFIED, FILE, why));
    let specific = match fail.kind {
        Kind::Schema => Some(codes::LAUNCHER_SCHEMA_UNKNOWN),
        Kind::MultipleActive => Some(codes::LAUNCHER_MULTIPLE_ACTIVE),
        Kind::NoActive => Some(codes::LAUNCHER_NO_ACTIVE),
        _ => None,
    };
    if let Some(code) = specific {
        diags.push(IngestDiagnostic::error(code, FILE, fail.detail.clone()));
    }
    let detail = fail.detail.into_boxed_str();
    match fail.kind {
        Kind::Corrupt => OrderError::Corrupt { detail },
        Kind::Schema => OrderError::SchemaUnknown { found: detail },
        _ => OrderError::Unreadable { detail },
    }
}

impl LoadOrderSource for LauncherV2Sqlite {
    fn id(&self) -> &'static str {
        ID
    }

    /// `Exact` for both, **only when the snapshot verified**.
    fn ceiling(&self) -> (Confidence, Confidence) {
        (Confidence::Exact, Confidence::Exact)
    }

    fn try_load(&self, loc: &ResolvedLocations) -> Result<Playset, OrderError> {
        self.run(loc).attempt.result
    }
}

impl ChainSource for LauncherV2Sqlite {
    fn load(&self, loc: &ResolvedLocations) -> SourceAttempt {
        self.run(loc).attempt
    }

    fn load_with(&self, loc: &ResolvedLocations, found: &Discovery) -> SourceAttempt {
        self.run_with(loc, found).attempt
    }
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_data;
