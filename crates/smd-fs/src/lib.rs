//! `smd-fs` — path normalisation, the secure read-only walk, and content hashing.
//!
//! **Owns:** path normalisation, the read-only root handle, the link-refusing walk, and
//! `blake3` hashing. (OS default discovery and `libraryfolders.vdf` arrive later.)
//!
//! **Must never:** Know what any particular game is.
//!
//! Trust boundary (doc 05, "Filesystem"): every byte below a root is untrusted. Links are
//! never followed, FIFOs and devices are never opened, type/size/mtime come from the
//! opened handle, and every bound that trips is a diagnostic plus an incompleteness.

#![forbid(unsafe_code)]

mod hashing;
mod hostpath;
mod limits;
mod normpath;
mod record;
mod root;
mod sys;
mod walk;

pub use hashing::hash_reader;
pub use hostpath::{same_host_dir, same_host_dir_with};
pub use limits::WalkLimits;
pub use normpath::{NormError, NormPath, shipped_name};
pub use record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
pub use root::ReadOnlyRoot;
