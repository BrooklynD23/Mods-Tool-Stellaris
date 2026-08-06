//! `smd-index` — Overlay table, symbol table, resolver, interners, and the on-disk cache.
//!
//! **Owns:** Overlay table, symbol table, reverse indices, resolver, interners, on-disk cache.
//!
//! **Must never:** Interpret meaning or assign risk.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
