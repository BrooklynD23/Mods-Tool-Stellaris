//! `smd-rules` — The rule engine and one file per rule.
//!
//! **Owns:** `Rule` trait, engine, one rule per file, per-rule `evidence()` and `cases()`.
//!
//! **Must never:** Format output, hardcode a score, or name a game.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
