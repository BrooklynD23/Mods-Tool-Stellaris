//! `smd-gui` — The desktop shell adapter over the engine.
//!
//! **Owns:** The desktop shell and its IPC glue, over `smd-api` and `smd-report`. Which shell is decision D1, unsettled until the M7 checkpoint; nothing here may assume the answer.
//!
//! **Must never:** Contain domain logic. The engine is a library first; the GUI is a thin adapter.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
