//! Finding the game, the user directory and every workshop root on this machine (doc 05,
//! § Discovery).
//!
//! Precedence, highest first: explicit flag, `SMD_*` environment variable, a Steam library,
//! the Windows registry, the OS default. The first candidate that exists wins, and every
//! candidate tried is recorded with why it was tried, so the user can see what was picked
//! and what was passed over. Nothing is guessed silently, and nothing is written.
//!
//! The machine is injected ([`HostEnv`]); no global is read here. Every name that belongs
//! to a game (app id, folder names, executable, registry display name) comes from the
//! [`Profile`].

use std::path::{Path, PathBuf};

use smd_fs::{HostEnv, Origin};
use smd_profiles::Profile;

use crate::codes::{self, IngestDiagnostic};

mod dirs;
mod game;
mod steam;

#[cfg(test)]
mod tests;

/// Which storefront the located game came from.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum Store {
    /// Found in a Steam library.
    Steam,
    /// Found through an installer's uninstall entry (GOG and similar).
    Gog,
    /// Found under an Xbox / Game Pass `XboxGames` root.
    Xbox,
    /// Given by the user (flag or environment) or otherwise not attributable.
    Other,
    /// No game directory was found.
    Unknown,
}

impl Store {
    /// The lowercase name used in output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Store::Steam => "steam",
            Store::Gog => "gog",
            Store::Xbox => "xbox",
            Store::Other => "other",
            Store::Unknown => "unknown",
        }
    }
}

/// One place that was tried.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Candidate {
    /// What it was a candidate for: `game_dir`, `user_dir`, `workshop_dir`, `steam_root`
    /// or `steam_library`.
    pub key: &'static str,
    /// The path tried.
    pub path: PathBuf,
    /// Why it was tried.
    pub origin: Origin,
    /// Whether it exists as a directory.
    pub exists: bool,
}

/// What the caller states outright. Anything left empty is searched for.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Overrides {
    /// `--game-dir`.
    pub game_dir: Option<PathBuf>,
    /// `--user-dir`.
    pub user_dir: Option<PathBuf>,
    /// `--workshop-dir`, once per occurrence. When non-empty, no workshop is searched for.
    pub workshop_dirs: Vec<PathBuf>,
}

/// Everything location discovery found.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Located {
    /// The store the game directory came from.
    pub store: Store,
    /// The game install directory.
    pub game_dir: Option<PathBuf>,
    /// The user data directory.
    pub user_dir: Option<PathBuf>,
    /// Every workshop content directory that exists, one per Steam library that has one.
    pub workshop_dirs: Vec<PathBuf>,
    /// Every Steam library found, existing only.
    pub steam_libraries: Vec<PathBuf>,
    /// Every place tried, in the order tried.
    pub candidates: Vec<Candidate>,
    /// Why something is missing, or was passed over.
    pub diagnostics: Vec<IngestDiagnostic>,
}

/// The state of one `locate` call.
pub(crate) struct Run<'a> {
    pub profile: &'a Profile,
    pub env: &'a dyn HostEnv,
    pub candidates: Vec<Candidate>,
    pub diags: Vec<IngestDiagnostic>,
}

impl Run<'_> {
    /// Record a candidate; its `exists` is whether it is a directory.
    pub(crate) fn try_dir(&mut self, key: &'static str, path: PathBuf, origin: Origin) -> bool {
        let exists = path.is_dir();
        self.record(key, path, origin, exists);
        exists
    }

    pub(crate) fn record(
        &mut self,
        key: &'static str,
        path: PathBuf,
        origin: Origin,
        exists: bool,
    ) {
        self.candidates.push(Candidate { key, path, origin, exists });
    }

    /// An override that is named but absent.
    pub(crate) fn missing_override(&mut self, key: &str, origin: Origin, path: &Path) {
        let why = format!("{} given by {} does not exist", path.display(), origin.as_str());
        self.diags.push(IngestDiagnostic::warn(codes::LOCATE_OVERRIDE_MISSING, key, why));
    }

    pub(crate) fn unresolved(&mut self, key: &str) {
        let tried = self.candidates.iter().filter(|c| c.key == key).count();
        let why = format!("not found; {tried} candidate(s) tried (see candidates)");
        self.diags.push(IngestDiagnostic::warn(codes::LOCATE_UNRESOLVED, key, why));
    }
}

/// Whether `list` already holds the directory `p`: spelled the same, or the same after
/// links are resolved (`~/.steam/steam` is a link to the real Steam root). Only the
/// comparison uses the resolved form; callers keep the path as given.
pub(crate) fn contains_dir(list: &[PathBuf], p: &Path) -> bool {
    let canon = |x: &Path| std::fs::canonicalize(x).unwrap_or_else(|_| x.to_path_buf());
    let real = canon(p);
    list.iter().any(|q| smd_fs::same_host_dir(q, p) || smd_fs::same_host_dir(&canon(q), &real))
}

/// Add `p` unless [`contains_dir`] already has it.
pub(crate) fn push_unique(list: &mut Vec<PathBuf>, p: PathBuf) {
    if !contains_dir(list, &p) {
        list.push(p);
    }
}

/// Find the game, the user directory and every workshop root.
#[must_use]
pub fn locate(profile: &Profile, overrides: &Overrides, env: &dyn HostEnv) -> Located {
    let mut run = Run { profile, env, candidates: Vec::new(), diags: Vec::new() };
    let libs = run.steam_libraries();
    let (game_dir, store) = run.game_dir(overrides.game_dir.as_deref(), &libs);
    let user_dir = run.user_dir(overrides.user_dir.as_deref(), &libs);
    let workshop_dirs = run.workshop_dirs(&overrides.workshop_dirs, &libs);
    Located {
        store,
        game_dir,
        user_dir,
        workshop_dirs,
        steam_libraries: libs,
        candidates: run.candidates,
        diagnostics: run.diags,
    }
}
