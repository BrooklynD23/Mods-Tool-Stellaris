//! `smd-ingest` — Mod discovery, descriptors, and load-order sources.
//!
//! **Owns:** `.mod`/`descriptor.mod` parsing, workshop scan, the name→impl registry of `LoadOrderSource`s.
//!
//! **Must never:** Parse mod *content*.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
