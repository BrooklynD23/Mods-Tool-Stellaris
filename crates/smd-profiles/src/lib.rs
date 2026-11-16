//! `smd-profiles` — Game knowledge as data: the `GameProfile` trait, its TOML schema, and the shipped profiles.
//!
//! **Owns:** `GameProfile` trait, TOML schema, the shipped profile data, `SourceSpec` names + config, per-profile `FeatureTag` declarations.
//!
//! **Must never:** Contain analysis logic; name or construct a load-order *implementation*. It names them as strings and `smd-ingest` resolves them (ADR-0002 § 1).
//!
//! Stub. This crate is scaffolding at M0: the workspace, the layer map, and the size and
//! dependency-direction gates are live from commit one so that nothing lands before the
//! rules that constrain it. Its contents arrive at the milestone named in
//! `docs/architecture/15-roadmap.md`.

#![forbid(unsafe_code)]
