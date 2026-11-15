//! `smd-fs` — path normalisation, the secure read-only walk, and content hashing.
//!
//! **Owns:** path normalisation, the read-only root handle, the link-refusing walk, `blake3`
//! hashing, and the OS primitives location discovery needs: Steam KeyValues reading, registry
//! reads through `reg.exe`, and the Documents / Steam-root lookups, all behind [`HostEnv`].
//!
//! **Must never:** Know what any particular game is.
//!
//! Trust boundary (doc 05, "Filesystem"): every byte below a root is untrusted. Links are
//! never followed, FIFOs and devices are never opened, type/size/mtime come from the
//! opened handle, and every bound that trips is a diagnostic plus an incompleteness.

#![forbid(unsafe_code)]

mod hashing;
mod host;
mod hostpath;
mod limits;
mod normpath;
mod record;
mod registry;
mod root;
mod steam_vdf;
mod sys;
mod walk;

pub use hashing::hash_reader;
pub use host::{
    HostEnv, MapEnv, Origin, Os, SystemEnv, documents_dir, home_dir, steam_roots, xdg_data_home,
};
pub use hostpath::{same_host_dir, same_host_dir_with};
pub use limits::WalkLimits;
pub use normpath::{NormError, NormPath, shipped_name};
pub use record::{FsDiagnostic, Incomplete, WalkOutcome, WalkRecord, codes};
pub use registry::{RegValue, expand_percent_vars, parse_reg_output};
pub use root::ReadOnlyRoot;
pub use steam_vdf::{
    DEPTH_MAX as VDF_DEPTH_MAX, FILE_MAX as VDF_FILE_MAX, LIBRARIES_MAX as VDF_LIBRARIES_MAX,
    VdfError, parse_install_dir, parse_library_folders,
};
