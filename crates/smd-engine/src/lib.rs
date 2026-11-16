//! `smd-engine` — Orchestration: the pipeline, the thread pool, and the job supervisor.
//!
//! **Owns:** Stage wiring, the thread pool, channels, cancellation, progress, error collection.
//!
//! **Must never:** Contain domain rules.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
