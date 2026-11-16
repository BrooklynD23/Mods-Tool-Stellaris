//! `smd-engine` — Orchestration: the pipeline, the thread pool, and the job supervisor.
//!
//! **Owns:** Stage wiring, the thread pool, channels, cancellation, progress, error collection.
//!
//! **Must never:** Contain domain rules.
//!
//! # What exists so far
//!
//! [`scan`]: ingest discovery and the order chain, a walk of every source that is not
//! `Inactive`, the index, the registered rules run sequentially, each finding's
//! [`Certainty`](smd_core::Certainty) computed here as the meet of what its participants
//! support and what the rule supplied, and the result as text ([`ScanOutput`]).
//!
//! Not yet: the rayon pool, progress events, the job supervisor, an explicit order file,
//! more than one workshop directory.

#![forbid(unsafe_code)]

mod certainty;
mod output;
mod request;
mod rows;
mod scan;
#[cfg(test)]
mod tests;
mod walk;

pub use output::{ModRow, ScanDiagnostic, ScanOutput};
pub use request::{ScanError, ScanRequest};
pub use scan::scan;
pub use smd_fs::WalkLimits;
pub use smd_rules::CancellationToken;
