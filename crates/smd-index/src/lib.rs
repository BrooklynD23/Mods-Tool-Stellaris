//! `smd-index` — Overlay table, symbol table, resolver, interners, and the on-disk cache.
//!
//! **Owns:** Overlay table, symbol table, reverse indices, resolver, interners, on-disk cache.
//!
//! **Must never:** Interpret meaning or assign risk.
//!
//! # What exists so far
//!
//! The file layer (doc 07 § Stage A, § Stage B): [`IndexBuilder`] takes the source table,
//! per-source walk records and an [`OverlayRules`], interns paths in ascending byte order
//! (so a `PathId` is a function of the set of paths, never of walk order), builds the
//! [`OverlayTable`], and derives [`Derived`]. [`ScanIndex::resolve_path`] is Stage A;
//! [`ScanIndex::resolve_finding`] turns a finding into text.
//!
//! Not yet: the definition, localisation and event tables, `Precedence::compare`
//! (Stages C and D), `PathClassTable`, `reorder`, and the parse cache.
//!
//! The crate names no game and no directory: per-path `same_path` behaviour and the
//! `replace_path_scope` reading arrive as [`OverlayRules`].

#![forbid(unsafe_code)]

mod build;
mod derived;
pub mod fixture;
mod index;
mod interner;
mod overlay;
mod overlay_rules;
mod replace;
mod resolve;
mod stage_a;

#[cfg(test)]
mod tests;

pub use build::{FileRecord, IndexBuilder, SourceInput};
pub use derived::{BitSet, Derived};
pub use fixture::{FileShape, PlaysetBuilder};
pub use index::ScanIndex;
pub use interner::{Interner, InternerBuilder};
pub use overlay::{Contribution, OverlayTable};
pub use overlay_rules::{OverlayRules, ReplacePathScope};
pub use replace::{ReplaceDecl, covers, normalise_prefix};
pub use stage_a::PathVerdict;
