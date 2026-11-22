//! `smd-api` — The stable command/query/event surface, and its TypeScript export.
//!
//! **Owns:** `Command`/`Query`/`Event` enums, serde + TS export.
//!
//! **Must never:** Be imported by anything below it.
//!
//! # What exists so far
//!
//! Three commands: [`Command::Scan`] with its text-only [`ScanOutcome`] (scored only
//! when the request opts in), [`Command::Locate`] with [`LocateOutput`], and
//! [`Command::Suppress`], which edits the user suppressions file. Nothing here carries
//! a `SourceId`, `PathId` or `SymbolId` (Invariant C1): an outcome is mod keys, display
//! names, paths as text, and [`ResolvedFinding`]s. The adapters (`smd-cli`, later the GUI
//! and Python bindings) see the engine only through this crate, so the request and
//! diagnostic types they need are re-exported here.
//!
//! Not yet: queries, events, serde, and the TypeScript export.

#![forbid(unsafe_code)]

use smd_core::{PlaysetProvenance, ResolvedFinding, ScanCompleteness};
use smd_engine::ScanOutput;

pub use smd_engine::{
    Annotation, CancellationToken, Candidate, Explanation, LocateOutput, LocateRequest, LocatedMod,
    ModRow, Overrides, Progress, ProgressFn, ScanDiagnostic, ScanError, ScanRequest, Store,
    WalkLimits,
};

/// Something the engine can be asked to do.
#[derive(Clone, Debug)]
pub enum Command {
    /// Scan an install and its mods for same-path conflicts.
    Scan(ScanRequest),
    /// Find the game's directories on this machine and every mod under them.
    Locate(LocateRequest),
    /// Add a finding id to the user suppressions file.
    Suppress(SuppressRequest),
}

/// What [`Command::Suppress`] needs.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SuppressRequest {
    /// The finding id `smd scan --score` printed.
    pub id: String,
    /// Why, kept in the file for the user.
    pub note: Option<String>,
    /// The file to edit; `None` is the OS config-dir default.
    pub path: Option<std::path::PathBuf>,
}

/// What a command produced.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    /// The result of [`Command::Scan`].
    Scan(Box<ScanOutcome>),
    /// The result of [`Command::Locate`].
    Locate(Box<LocateOutput>),
    /// The result of [`Command::Suppress`]: the file written.
    Suppressed(std::path::PathBuf),
}

/// A finished scan, as text.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanOutcome {
    /// Where the order came from and how far it and the activation are trusted.
    pub provenance: PlaysetProvenance,
    /// What was not fully read. A scan that was not complete never means "nothing found".
    pub completeness: ScanCompleteness,
    /// One row per source, vanilla first.
    pub inventory: Vec<ModRow>,
    /// Sorted with `smd-core`'s text sort.
    pub findings: Vec<ResolvedFinding>,
    /// Scan diagnostics.
    pub diagnostics: Vec<ScanDiagnostic>,
    /// Empty unless the scan was scored; then `annotations[i]` is `findings[i]`'s.
    pub annotations: Vec<Annotation>,
}

impl From<ScanOutput> for ScanOutcome {
    fn from(o: ScanOutput) -> ScanOutcome {
        ScanOutcome {
            provenance: o.provenance,
            completeness: o.completeness,
            inventory: o.inventory,
            findings: o.findings,
            diagnostics: o.diagnostics,
            annotations: o.annotations,
        }
    }
}

/// Run a command to completion.
///
/// # Errors
/// [`ScanError`] when the request cannot be served; see its variants.
pub fn execute(command: Command) -> Result<Outcome, ScanError> {
    match command {
        Command::Scan(request) => {
            smd_engine::scan(&request).map(|o| Outcome::Scan(Box::new(o.into())))
        }
        Command::Locate(request) => {
            smd_engine::locate(&request).map(|o| Outcome::Locate(Box::new(o)))
        }
        Command::Suppress(r) => {
            smd_engine::suppress(&r.id, r.note, r.path.as_deref()).map(Outcome::Suppressed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_scan_command_surfaces_the_engine_error() {
        let dir = std::env::temp_dir();
        let mut req =
            ScanRequest::new(dir.join("smd-api-no-such-game"), dir.join("smd-api-no-such-user"));
        req.home = Some(dir);
        assert!(matches!(
            execute(Command::Scan(req)),
            Err(ScanError::RootUnreadable { role: "game", .. })
        ));
    }
}
