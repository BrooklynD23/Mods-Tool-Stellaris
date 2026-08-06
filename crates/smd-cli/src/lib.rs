//! `smd-cli` — The command-line adapter over the engine.
//!
//! **Owns:** Argument parsing and terminal output, over `smd-api` and `smd-report`.
//!
//! **Must never:** Contain domain logic. The engine is a library first; the CLI is a thin adapter.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
