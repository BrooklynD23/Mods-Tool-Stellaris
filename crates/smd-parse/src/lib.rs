//! `smd-parse` — Lexers, the parser registry, and fact emission.
//!
//! **Owns:** Lexers for PDX script and localisation, the `FileParser` registry, dispatch by `FileKind`, fact emission.
//!
//! **Must never:** Know about traits, civics, or any game concept; store anything.
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
