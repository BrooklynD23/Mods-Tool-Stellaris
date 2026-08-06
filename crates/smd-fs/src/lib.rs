//! `smd-fs` — OS path resolution, path normalisation, and content hashing.
//!
//! **Owns:** OS path resolution, `libraryfolders.vdf`, path normalisation, `blake3` hashing.
//!
//! **Must never:** Know what any particular game is.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
