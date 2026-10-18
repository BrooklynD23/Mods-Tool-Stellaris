//! The typed reader for the keys Stage A needs.

use serde::Deserialize;
use smd_core::{SamePath, SourceSpec};

use crate::glob;

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
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileError::Syntax(e) => write!(f, "profile is not valid: {e}"),
            ProfileError::Value { key, found } => {
                write!(f, "profile key `{key}` has the unknown value `{found}`")
            }
        }
    }
}

impl std::error::Error for ProfileError {}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    game: RawGame,
    #[serde(default)]
    load_order: RawLoadOrder,
    #[serde(default)]
    dir: Vec<RawDir>,
    #[serde(default)]
    benign: RawBenign,
}

#[derive(Deserialize, Default)]
struct RawGame {
    id: Option<String>,
}

#[derive(Deserialize, Default)]
struct RawLoadOrder {
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    same_filename_replaces_whole_file: bool,
    replace_path_scope: Option<String>,
}

#[derive(Deserialize)]
struct RawDir {
    glob: String,
    same_path: Option<String>,
}

#[derive(Deserialize, Default)]
struct RawBenign {
    #[serde(default)]
    paths: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Dir {
    glob: Box<str>,
    same_path: Option<SamePath>,
}

/// The parts of a game profile that Stage A reads.
///
/// Every other key is ignored for now. Strict rejection of unknown keys (REVIEW-2026-10
/// H6) belongs to the stage-keyed loader that replaces this reader; until then a typo in
/// a key this reader does not model is silently dropped.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Profile {
    id: Box<str>,
    sources: Vec<Box<str>>,
    same_filename_replaces: bool,
    scope: ReplacePathScope,
    dirs: Vec<Dir>,
    benign: Vec<Box<str>>,
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

impl Profile {
    /// Read a profile from TOML text.
    ///
    /// # Errors
    /// [`ProfileError`] when the text is not TOML, or a modelled key has an unknown value.
    pub fn from_toml(text: &str) -> Result<Profile, ProfileError> {
        let raw: Raw = toml::from_str(text).map_err(|e| ProfileError::Syntax(e.to_string()))?;
        let dirs = raw
            .dir
            .into_iter()
            .map(|d| {
                let same_path = d.same_path.as_deref().map(|v| same_path("same_path", v));
                Ok(Dir { glob: d.glob.to_lowercase().into(), same_path: same_path.transpose()? })
            })
            .collect::<Result<Vec<_>, ProfileError>>()?;
        Ok(Profile {
            id: raw.game.id.unwrap_or_default().into(),
            sources: raw.load_order.sources.into_iter().map(Into::into).collect(),
            same_filename_replaces: raw.load_order.same_filename_replaces_whole_file,
            scope: scope(raw.load_order.replace_path_scope.as_deref())?,
            dirs,
            benign: raw.benign.paths.into_iter().map(|p| p.to_lowercase().into()).collect(),
        })
    }

    /// The profile's id, e.g. the `[game] id` value; empty when it declares none.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
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
    /// The most specific matching `[[dir]]` (longest literal prefix, then declaration
    /// order) decides; its own `same_path` wins, else the `[load_order]` default. A path
    /// under no `[[dir]]` takes that same default (doc 07 § Resolution model): `Replace`
    /// when `same_filename_replaces_whole_file`, else `Unknown`.
    ///
    /// Doc 10 also has the loader reject two entries that tie on specificity while
    /// overlapping; that check is deferred to H6, so a tie falls to declaration order.
    #[must_use]
    pub fn classify(&self, path: &str) -> SamePath {
        let best = self.dirs.iter().filter(|d| glob::matches(&d.glob, path)).fold(
            None::<&Dir>,
            |best, d| match best {
                Some(b) if glob::specificity(&b.glob) >= glob::specificity(&d.glob) => Some(b),
                _ => Some(d),
            },
        );
        let default =
            if self.same_filename_replaces { SamePath::Replace } else { SamePath::Unknown };
        best.and_then(|d| d.same_path).unwrap_or(default)
    }

    /// Whether `[benign] paths` lists this path: never worth reporting.
    #[must_use]
    pub fn is_benign(&self, path: &str) -> bool {
        self.benign.iter().any(|p| glob::matches_basename_or_path(p, path))
    }
}
