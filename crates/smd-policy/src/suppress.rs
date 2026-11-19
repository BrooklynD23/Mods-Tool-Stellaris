//! User suppressions: a versioned, human-editable TOML file under the OS config dir.
//!
//! A suppression names one finding id, which is a hash of the rule and the subject — so
//! it hides exactly the finding it was made for, never a coarser set (doc 09 §
//! Suppression predicates). It is applied after scoring and never removes a finding: the
//! finding is still reported, marked suppressed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::PolicyError;

/// The file format version this build reads and writes.
pub const SUPPRESSIONS_VERSION: u32 = 1;

/// One suppressed finding.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Suppression {
    /// The finding id `smd scan --score` prints.
    pub id: String,
    /// Why, in the user's words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The whole file.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Suppressions {
    /// Format version; a file from a newer build is refused rather than misread.
    pub version: u32,
    /// The entries, in the order they were added.
    #[serde(default, rename = "suppress")]
    pub entries: Vec<Suppression>,
}

impl Default for Suppressions {
    fn default() -> Self {
        Suppressions { version: SUPPRESSIONS_VERSION, entries: Vec::new() }
    }
}

/// A finding id: 16 lowercase hex digits.
#[must_use]
pub fn is_finding_id(id: &str) -> bool {
    id.len() == 16 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `%APPDATA%\<app>\suppressions.toml` on Windows; `$XDG_CONFIG_HOME/<app>/` (or
/// `~/.config/<app>/`) elsewhere. `None` when no such directory can be named.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        env("APPDATA")?.join(crate::APP_DIR_WINDOWS)
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))?
            .join(crate::APP_DIR_XDG)
    };
    Some(dir.join("suppressions.toml"))
}

impl Suppressions {
    /// Read `path`; a missing file is an empty set.
    ///
    /// # Errors
    /// [`PolicyError`] when the file cannot be read, is not valid, or is a newer version.
    pub fn load(path: &Path) -> Result<Suppressions, PolicyError> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(PolicyError::Io(format!("{}: {e}", path.display()))),
        };
        let s: Suppressions = toml::from_str(&text)
            .map_err(|e| PolicyError::Syntax(format!("{}: {e}", path.display())))?;
        if s.version > SUPPRESSIONS_VERSION {
            let why =
                format!("{}: version {} is newer than this build reads", path.display(), s.version);
            return Err(PolicyError::Invalid(why));
        }
        Ok(s)
    }

    /// Whether `id` is suppressed.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    /// A copy with `id` added; unchanged when it is already present.
    ///
    /// # Errors
    /// [`PolicyError::Invalid`] when `id` is not a finding id.
    pub fn with(&self, id: &str, note: Option<String>) -> Result<Suppressions, PolicyError> {
        if !is_finding_id(id) {
            return Err(PolicyError::Invalid(format!(
                "`{id}` is not a finding id (16 hex digits)"
            )));
        }
        let mut next = self.clone();
        if !next.contains(id) {
            next.entries.push(Suppression { id: id.to_owned(), note });
        }
        Ok(next)
    }

    /// Write to `path`, creating its directory, through a temporary file and a rename so a
    /// crash never leaves half a file.
    ///
    /// # Errors
    /// [`PolicyError::Io`] when the directory or file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), PolicyError> {
        let io = |e: std::io::Error| PolicyError::Io(format!("{}: {e}", path.display()));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let body = toml::to_string(self).map_err(|e| PolicyError::Io(e.to_string()))?;
        let text = format!(
            "# Suppressed findings. Edit freely; ids come from `smd scan --score`.\n{body}"
        );
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, path).map_err(io)
    }
}
