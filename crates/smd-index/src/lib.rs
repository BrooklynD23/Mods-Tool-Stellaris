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
//! The definition layer (doc 07 § Stage C, § Stage D, § Deleted symbols, M2):
//! [`IndexBuilder::add_facts`] interns each file's `RawFact`s into a [`DefinitionTable`];
//! [`ScanIndex::resolve_symbol`] resolves one key through Stages C and D under every
//! reading of the unknown Stage D parameters, and [`ScanIndex::no_live_site`] lists the
//! keys `core.symbol_deleted` examines. Per-path classification arrives as
//! [`SymbolRules`].
//!
//! The localisation layer (M4): `LocKey` facts go into a [`LocTable`] keyed
//! `(LangId, key)`; [`ScanIndex::resolve_loc`] compares the class `tier` first (H3), then
//! the same Stage D as definitions.
//!
//! Not yet: the event table, `tier` for definitions, a separate merge scope (the domain is
//! the scope), the `winning` bitset, `reorder`, and the on-disk cache.
//!
//! The crate names no game and no directory: per-path `same_path` behaviour and the
//! `replace_path_scope` reading arrive as [`OverlayRules`].

#![forbid(unsafe_code)]

mod build;
mod defs;
mod derived;
pub mod fixture;
mod index;
mod interner;
mod loc;
mod meta;
mod overlay;
mod overlay_rules;
mod replace;
mod resolve;
mod stage_a;

#[cfg(test)]
mod tests;

pub use build::{FileRecord, IndexBuilder, SourceInput};
pub use defs::{DefSite, DefinitionTable, IncompleteFile, PathClass, SymbolRules, SymbolVerdict};
pub use derived::{BitSet, Derived};
pub use fixture::{FIXTURE_DOMAIN, FIXTURE_LANG, FileShape, PlaysetBuilder};
pub use index::ScanIndex;
pub use interner::{Interner, InternerBuilder};
pub use loc::{LocSite, LocTable, LocVerdict};
pub use meta::{ModDescriptors, PlaysetMeta, pattern_text, version_text};
pub use overlay::{Contribution, OverlayTable};
pub use overlay_rules::{OverlayRules, ReplacePathScope};
pub use replace::{ReplaceDecl, covers, normalise_prefix};
pub use stage_a::PathVerdict;
