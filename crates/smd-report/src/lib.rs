//! `smd-report` — Renderers over `ResolvedFinding`: json, markdown, html, table.
//!
//! **Owns:** `Renderer` trait + impls over `ResolvedFinding`.
//!
//! **Must never:** Compute findings; depend on anything but `smd-core`.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
