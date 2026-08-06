//! `smd-api` — The stable command/query/event surface, and its TypeScript export.
//!
//! **Owns:** `Command`/`Query`/`Event` enums, serde + TS export.
//!
//! **Must never:** Be imported by anything below it.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
