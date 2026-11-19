//! The typed reader for the keys Stage A needs.

use serde::Deserialize;
use smd_core::{SamePath, SourceSpec};

use crate::dirclass::{DirClass, KeyReadings, RawExtract, RawReadings};
use crate::glob;
use crate::locations::{Locations, Stores};
use crate::strict::{self, Rest};
use crate::version::{RawVersion, VersionSpec};

/// What a `replace_path` drops, as the profile states it. Mirrors the resolver's own enum
/// by name; `smd-engine` maps between them, because this crate sits below the index.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ReplacePathScope {
    /// Not established.
    #[default]
    Unknown,
    /// Only the base game's files are dropped.
    VanillaOnly,
    /// The base game's files and every earlier mod's.
    VanillaAndEarlier,
}

/// Why a profile could not be read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ProfileError {
    /// The text is not TOML of the expected shape.
    Syntax(String),
    /// A key this reader models has a value it does not know.
    Value {
        /// The key.
        key: &'static str,
        /// The value found.
        found: String,
    },
    /// A key the loader does not recognise (doc 10: a load error, not a warning).
    UnknownKey {
        /// The table, without brackets.
        table: &'static str,
        /// The key found.
        key: String,
    },
    /// Two `[[dir]]` globs tie on specificity and match a common path.
    OverlappingGlobs {
        /// The earlier glob.
        first: String,
        /// The later glob.
        second: String,
    },
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::Syntax(e) => write!(f, "profile is not valid: {e}"),
            ProfileError::Value { key, found } => {
                write!(f, "profile key `{key}` has the unknown value `{found}`")
            }
            ProfileError::UnknownKey { table, key } => {
                write!(f, "profile table `[{table}]` has the unknown key `{key}`")
            }
            ProfileError::OverlappingGlobs { first, second } => write!(
                f,
                "profile `[[dir]]` globs `{first}` and `{second}` overlap with equal specificity"
            ),
        }
    }
}

impl std::error::Error for ProfileError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    game: RawGame,
    #[serde(default)]
    locations: Locations,
    #[serde(default)]
    stores: Stores,
    #[serde(default)]
    load_order: RawLoadOrder,
    #[serde(default)]
    syntax: Rest,
    #[serde(default)]
    dir: Vec<RawDir>,
    #[serde(default)]
    benign: RawBenign,
    #[serde(default)]
    asset_archives: RawAssetArchives,
    #[serde(default)]
    version: RawVersion,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawGame {
    id: Option<String>,
    // Display-only; read so that `[game]` can reject unknown keys.
    #[allow(dead_code)]
    name: Option<String>,
    steam_id: Option<u32>,
}

#[derive(Deserialize, Default)]
struct RawLoadOrder {
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    same_filename_replaces_whole_file: bool,
    replace_path_scope: Option<String>,
    #[serde(flatten)]
    readings: RawReadings,
    #[serde(flatten)]
    rest: Rest,
}

#[derive(Deserialize)]
struct RawDir {
    glob: String,
    same_path: Option<String>,
    #[serde(flatten)]
    extract: RawExtract,
    #[serde(flatten)]
    rest: Rest,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawBenign {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawAssetArchives {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Dir {
    glob: Box<str>,
    same_path: Option<SamePath>,
    class: DirClass,
}

/// The parts of a game profile that Stage A reads.
///
/// Every table rejects unknown keys, and tied overlapping `[[dir]]` globs are a load error
/// (REVIEW-2026-10 H6, [`crate::strict`]). Keys owned by later stages are recognised but
/// not yet typed here.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Profile {
    id: Box<str>,
    steam_id: Option<u32>,
    locations: Locations,
    stores: Stores,
    sources: Vec<Box<str>>,
    same_filename_replaces: bool,
    scope: ReplacePathScope,
    dirs: Vec<Dir>,
    benign: Vec<Box<str>>,
    asset_archives: Vec<Box<str>>,
    version: VersionSpec,
    readings: KeyReadings,
    domains: Vec<Box<str>>,
    languages: std::sync::Arc<[Box<str>]>,
    hash: [u8; 16],
}

fn same_path(key: &'static str, v: &str) -> Result<SamePath, ProfileError> {
    match v {
        "replace" => Ok(SamePath::Replace),
        "unknown" => Ok(SamePath::Unknown),
        other => Err(ProfileError::Value { key, found: other.to_owned() }),
    }
}

fn scope(v: Option<&str>) -> Result<ReplacePathScope, ProfileError> {
    match v {
        None | Some("unknown") => Ok(ReplacePathScope::Unknown),
        Some("vanilla_only") => Ok(ReplacePathScope::VanillaOnly),
        Some("vanilla_and_earlier") => Ok(ReplacePathScope::VanillaAndEarlier),
        Some(other) => {
            Err(ProfileError::Value { key: "replace_path_scope", found: other.to_owned() })
        }
    }
}

fn content_hash(text: &str) -> [u8; 16] {
    smd_fs::hash_reader(text.as_bytes(), text.len() as u64).map_or([0; 16], |(b, _)| b.hash)
}

/// Type every `[[dir]]`, assigning domain ids in ascending name order.
fn typed_dirs(
    raw: Vec<RawDir>,
    default_order: Option<smd_core::FileOrder>,
    languages: &std::sync::Arc<[Box<str>]>,
) -> Result<(Vec<Dir>, Vec<Box<str>>), ProfileError> {
    let mut domains: Vec<Box<str>> =
        raw.iter().filter_map(|d| d.extract.domain()).map(Into::into).collect();
    domains.sort();
    domains.dedup();
    let dirs = raw
        .into_iter()
        .map(|d| {
            let same_path = d.same_path.as_deref().map(|v| same_path("same_path", v));
            Ok(Dir {
                glob: d.glob.to_lowercase().into(),
                same_path: same_path.transpose()?,
                class: d.extract.typed(&domains, default_order, languages)?,
            })
        })
        .collect::<Result<Vec<_>, ProfileError>>()?;
    Ok((dirs, domains))
}

impl Profile {
    /// Read a profile from TOML text.
    ///
    /// # Errors
    /// [`ProfileError`] when the text is not TOML, or a modelled key has an unknown value.
    pub fn from_toml(text: &str) -> Result<Profile, ProfileError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ProfileError::Syntax(e.to_string()))?;
        raw.locations.check()?;
        strict::load_order(&raw.load_order.rest)?;
        strict::syntax(&raw.syntax)?;
        for d in &raw.dir {
            strict::dir(&d.rest)?;
        }
        let (readings, default_order) = raw.load_order.readings.typed()?;
        let languages = crate::dirclass::languages(&raw.syntax)?;
        let (dirs, domains) = typed_dirs(raw.dir, default_order, &languages)?;
        strict::no_tied_overlap(dirs.iter().map(|d| &*d.glob))?;
        Ok(Profile {
            readings,
            domains,
            languages,
            hash: content_hash(text),
            id: raw.game.id.unwrap_or_default().into(),
            steam_id: raw.game.steam_id,
            locations: raw.locations,
            stores: raw.stores,
            sources: raw.load_order.sources.into_iter().map(Into::into).collect(),
            same_filename_replaces: raw.load_order.same_filename_replaces_whole_file,
            scope: scope(raw.load_order.replace_path_scope.as_deref())?,
            dirs,
            benign: raw.benign.paths.into_iter().map(|p| p.to_lowercase().into()).collect(),
            version: VersionSpec::from_raw(raw.version)?,
            asset_archives: raw
                .asset_archives
                .paths
                .into_iter()
                .map(|p| p.to_lowercase().into())
                .collect(),
        })
    }

    /// `[version]`: where the game version is read from and the range checked.
    #[must_use]
    pub fn version(&self) -> &VersionSpec {
        &self.version
    }

    /// The profile's id, e.g. the `[game] id` value; empty when it declares none.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The Steam app id, when the profile declares one.
    #[must_use]
    pub fn steam_id(&self) -> Option<u32> {
        self.steam_id
    }

    /// `[locations]`: where the game's directories live, as templates.
    #[must_use]
    pub fn locations(&self) -> &Locations {
        &self.locations
    }

    /// `[stores]`: how to recognise non-Steam installs.
    #[must_use]
    pub fn stores(&self) -> &Stores {
        &self.stores
    }

    /// The load-order sources to try, in order, by registry name.
    #[must_use]
    pub fn load_order_sources(&self) -> Vec<SourceSpec> {
        self.sources.iter().map(|s| SourceSpec::named(s)).collect()
    }

    /// `[load_order] replace_path_scope`.
    #[must_use]
    pub fn replace_path_scope(&self) -> ReplacePathScope {
        self.scope
    }

    /// The `same_path` behaviour of a normalised path.
    ///
    /// The most specific matching `[[dir]]` (longest literal prefix) decides; the loader
    /// already rejected ties, so declaration order never does. Its own `same_path` wins,
    /// else the `[load_order]` default. A path under no `[[dir]]` takes that same default
    /// (doc 07 § Resolution model): `Replace` when `same_filename_replaces_whole_file`,
    /// else `Unknown`.
    #[must_use]
    pub fn classify(&self, path: &str) -> SamePath {
        let default =
            if self.same_filename_replaces { SamePath::Replace } else { SamePath::Unknown };
        self.best_dir(path).and_then(|d| d.same_path).unwrap_or(default)
    }

    /// The most specific matching `[[dir]]`, as [`classify`](Profile::classify) picks it.
    fn best_dir(&self, path: &str) -> Option<&Dir> {
        self.dirs.iter().filter(|d| glob::matches(&d.glob, path)).fold(None::<&Dir>, |best, d| {
            match best {
                Some(b) if glob::specificity(&b.glob) >= glob::specificity(&d.glob) => Some(b),
                _ => Some(d),
            }
        })
    }

    /// The extraction and key-layer classification of a normalised path: the most specific
    /// matching `[[dir]]`'s, or [`DirClass::default`] under none.
    #[must_use]
    pub fn dir_class(&self, path: &str) -> DirClass {
        self.best_dir(path).map(|d| d.class.clone()).unwrap_or_default()
    }

    /// Every declared domain name, ascending; a `DomainId` is a position in this list.
    #[must_use]
    pub fn domain_names(&self) -> &[Box<str>] {
        &self.domains
    }

    /// `[syntax] languages`: `LangId(i)` names `languages()[i]`.
    #[must_use]
    pub fn languages(&self) -> std::sync::Arc<[Box<str>]> {
        std::sync::Arc::clone(&self.languages)
    }

    /// The Stage D parameters of `[load_order]`.
    #[must_use]
    pub fn key_readings(&self) -> KeyReadings {
        self.readings
    }

    /// blake3 of the profile text, truncated: the `profile_hash` cache-key term (doc 06).
    #[must_use]
    pub fn content_hash(&self) -> [u8; 16] {
        self.hash
    }

    /// Whether `[benign] paths` lists this path: never worth reporting.
    #[must_use]
    pub fn is_benign(&self, path: &str) -> bool {
        self.benign.iter().any(|p| glob::matches_basename_or_path(p, path))
    }

    /// Whether `[asset_archives] paths` declares this path an asset-only archive: a
    /// container the profile states holds no script, so leaving it unread does not make
    /// a scan incomplete. The caller still reports that it was not read.
    #[must_use]
    pub fn is_asset_archive(&self, path: &str) -> bool {
        self.asset_archives.iter().any(|p| glob::matches_basename_or_path(p, path))
    }
}
