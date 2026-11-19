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
//! `sources`, each `[[dir]]`'s `glob`, optional `same_path`, and its extraction keys (`domain`, `merge`,
//! `level`, `parser`, `extractor`, `order`, `additive_children` — [`DirClass`]), the
//! Stage D `[load_order]` readings ([`KeyReadings`]), `[benign]` and `[asset_archives]`, plus `[game]`, `[locations]` and
//! `[stores]`. Every table rejects unknown keys, and `[[dir]]` globs that tie on
//! specificity while overlapping are a load error (REVIEW-2026-10 H6). Keys owned by later
//! stages are recognised by name but not yet typed. The shipped profile is embedded with the `profile-stellaris` feature
//! (on by default).

#![forbid(unsafe_code)]

mod dirclass;
mod glob;
mod locations;
mod profile;
mod strict;
#[cfg(test)]
mod tests;
mod version;

pub use dirclass::{DirClass, FilenameKey, KeyReadings};
/// Glob matching over normalised (lowercase, `/`-separated) paths, as profiles use it.
pub use glob::matches as glob_matches;
pub use locations::{Locations, Stores, expand};
pub use profile::{Profile, ProfileError, ReplacePathScope};
pub use version::{VersionSpec, parse_game_version, parse_pattern};

#[cfg(feature = "profile-stellaris")]
const SHIPPED_DEFAULT: &str = include_str!("../../../profiles/stellaris.toml");

/// Every profile this build embeds, as `(id, TOML text)`. The first is the default.
const SHIPPED: &[(&str, &str)] = &[
    #[cfg(feature = "profile-stellaris")]
    ("stellaris", SHIPPED_DEFAULT),
    #[cfg(feature = "profile-ck3")]
    ("ck3", include_str!("../../../profiles/ck3.toml")),
];

/// The ids of every profile this build embeds, default first.
#[must_use]
pub fn shipped_ids() -> Vec<&'static str> {
    SHIPPED.iter().map(|(id, _)| *id).collect()
}

impl Profile {
    /// The profile this build ships as its default.
    ///
    /// # Errors
    /// [`ProfileError`] if the embedded text does not read; a test pins that it does.
    #[cfg(feature = "profile-stellaris")]
    pub fn shipped_default() -> Result<Profile, ProfileError> {
        Profile::from_toml(SHIPPED_DEFAULT)
    }

    /// The embedded profile with this id; `None` picks the default (the first shipped).
    ///
    /// # Errors
    /// [`ProfileError::Value`] (key `game`) for an id this build does not ship, or the
    /// error of an embedded text that does not read.
    pub fn shipped(id: Option<&str>) -> Result<Profile, ProfileError> {
        let found = match id {
            None => SHIPPED.first(),
            Some(want) => SHIPPED.iter().find(|(i, _)| *i == want),
        };
        let unknown = || ProfileError::Value {
            key: "game",
            found: format!("{} (shipped: {})", id.unwrap_or(""), shipped_ids().join(", ")),
        };
        found.ok_or_else(unknown).and_then(|(_, text)| Profile::from_toml(text))
    }
}
