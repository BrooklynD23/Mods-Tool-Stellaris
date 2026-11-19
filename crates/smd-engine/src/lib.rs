//! `smd-engine` — Orchestration: the pipeline, the thread pool, and the job supervisor.
//!
//! **Owns:** Stage wiring, the thread pool, channels, cancellation, progress, error collection.
//!
//! **Must never:** Contain domain rules.
//!
//! # What exists so far
//!
//! [`scan`]: ingest discovery and the order chain, a walk of every source that is not
//! `Inactive`, a parse of every file the profile classes as script (cached in memory for
//! the scan), the index with its definition layer, the registered rules run sequentially, each finding's
//! [`Certainty`](smd_core::Certainty) computed here as the meet of what its participants
//! support and what the rule supplied, and the result as text ([`ScanOutput`]).
//!
//! [`locate`]: find the game, user and workshop directories on this machine and list every
//! mod under them.
//!
//! Not yet: the rayon pool, progress events, the job supervisor, an explicit order file.

#![forbid(unsafe_code)]

mod certainty;
mod hash_pass;
mod locate;
mod output;
mod parse_pass;
mod request;
mod rows;
mod scan;
#[cfg(test)]
mod tests;
mod walk;

pub use locate::{LocateOutput, LocateRequest, LocatedMod, locate, locate_with};
pub use output::{ModRow, ScanDiagnostic, ScanOutput};
pub use request::{Progress, ProgressFn, ScanError, ScanRequest};
pub use scan::scan;
pub use smd_fs::WalkLimits;
pub use smd_ingest::{Candidate, Located, Overrides, Store};
pub use smd_rules::CancellationToken;
