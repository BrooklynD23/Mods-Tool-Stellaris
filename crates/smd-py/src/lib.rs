//! `smd-py` — The optional PyO3 binding. Feature-gated, not in a default build.
//!
//! **Owns:** A scripting surface over `smd-api`, built by `maturin` as a secondary deliverable.
//!
//! **Must never:** Be depended on by anything. The GUI never depends on it, and it is a separate link unit with its own registry (ADR-0002 § 2a).
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
