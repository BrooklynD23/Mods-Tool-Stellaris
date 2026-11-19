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
//! `sources`, each `[[dir]]`'s `glob` and optional `same_path`, `[benign]` and `[asset_archives]`, plus `[game]`, `[locations]` and
//! `[stores]`. Every table rejects unknown keys, and `[[dir]]` globs that tie on
//! specificity while overlapping are a load error (REVIEW-2026-10 H6). Keys owned by later
//! stages are recognised by name but not yet typed. The shipped profile is embedded with the `profile-stellaris` feature
//! (on by default).

#![forbid(unsafe_code)]

mod glob;
mod locations;
mod profile;
mod strict;
#[cfg(test)]
mod tests;
mod version;

pub use locations::{Locations, Stores, expand};
pub use profile::{Profile, ProfileError, ReplacePathScope};
pub use version::{VersionSpec, parse_game_version, parse_pattern};

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
