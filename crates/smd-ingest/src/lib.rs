//! `smd-ingest` — Mod discovery, descriptors, and load-order sources.
//!
//! **Owns:** `.mod`/`descriptor.mod` parsing, workshop scan, the name→impl registry of `LoadOrderSource`s.
//!
//! **Must never:** Parse mod *content*.
//!
//! Everything here happens before a byte of mod content is read (doc 05). Every byte
//! below a root is untrusted: descriptors are read through `smd-fs`'s read-only root
//! handle under hard caps, and load-order sources fail loudly and fall through a chain
//! rather than guess.

#![forbid(unsafe_code)]

pub mod codes;
mod descriptor;
mod discovery;
mod inventory;
pub mod order;
#[cfg(test)]
mod testutil;

pub use codes::IngestDiagnostic;
pub use descriptor::{DEPTH_MAX, FILE_MAX, KeyValuesReader, LIST_MAX, READER_ID, VALUE_MAX};
pub use discovery::{
    ContentState, DiscoveredMod, Discovery, DiscoveryOptions, KEY_INSTALL, KEY_USER_DIR,
    KEY_WORKSHOP, MODS_SUBDIR, UnreadReason, discover,
};
pub use inventory::{Inventory, ModRecord, inventory};
pub use order::{
    ChainOutcome, ChainSource, DirectoryScan, DlcLoadJson, LauncherV2Options, LauncherV2Sqlite,
    OrderChain, SourceAttempt, SourceOptions, source_by_name,
};
