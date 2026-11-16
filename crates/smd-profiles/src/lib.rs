//! `smd-profiles` — Game knowledge as data: the `GameProfile` trait, its TOML schema, and the shipped profiles.
//!
//! **Owns:** `GameProfile` trait, TOML schema, the shipped profile data, `SourceSpec` names + config, per-profile `FeatureTag` declarations.
//!
//! **Must never:** Contain analysis logic; name or construct a load-order *implementation*. It names them as strings and `smd-ingest` resolves them (ADR-0002 § 1).
//!
//! # What exists so far
//!
//! A minimal typed reader, [`Profile`], for exactly what Stage A needs: the
//! `[load_order]` keys `same_filename_replaces_whole_file`, `replace_path_scope` and
//! `sources`, each `[[dir]]`'s `glob` and optional `same_path`, `[benign]` and `[asset_archives]`. Every other
//! key is ignored. Strict, stage-keyed loading with unknown-key rejection (REVIEW-2026-10
//! H6) replaces this reader; until then a mistyped key this reader does not model is
//! silently dropped. The shipped profile is embedded with the `profile-stellaris` feature
//! (on by default).

#![forbid(unsafe_code)]

mod glob;
mod profile;
#[cfg(test)]
mod tests;

pub use profile::{Profile, ProfileError, ReplacePathScope};

#[cfg(feature = "profile-stellaris")]
const SHIPPED_DEFAULT: &str = include_str!("../../../profiles/stellaris.toml");

impl Profile {
    /// The profile this build ships as its default.
    ///
    /// # Errors
    /// [`ProfileError`] if the embedded text does not read; a test pins that it does.
    #[cfg(feature = "profile-stellaris")]
    pub fn shipped_default() -> Result<Profile, ProfileError> {
        Profile::from_toml(SHIPPED_DEFAULT)
    }
}
