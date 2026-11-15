//! `smd locate`: find the game's directories on this machine, then list every mod under
//! them. Read-only; nothing here parses mod content.

use std::path::PathBuf;

use smd_core::{ResolvedLocations, SourceRole, VersionComponent, VersionPattern};
use smd_fs::{HostEnv, SystemEnv};
use smd_ingest::{
    ContentState, DiscoveredMod, DiscoveryOptions, IngestDiagnostic, KEY_INSTALL, KEY_USER_DIR,
    Located, Overrides, Store, check_game_version, codes, discover, workshop_key,
};
use smd_profiles::Profile;

use crate::output::ScanDiagnostic;
use crate::request::ScanError;
use crate::rows;

/// What to locate: the caller's explicit paths, and the home discovery must not swallow.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct LocateRequest {
    /// Explicit paths; anything left empty is searched for.
    pub overrides: Overrides,
    /// The home directory descriptors may not swallow. `None` reads `HOME` / `USERPROFILE`.
    pub home: Option<PathBuf>,
}

/// One mod found under the located directories.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocatedMod {
    /// Stable identity.
    pub key: String,
    /// Display name.
    pub name: String,
    /// The mod's own version string.
    pub version: Option<String>,
    /// `supported_version`, as written (`3.13.*`).
    pub supported_version: Option<String>,
    /// `vanilla`, `dlc`, `workshop`, `local` or `archive`.
    pub role: &'static str,
    /// The launcher `*.mod` file name, when there is one.
    pub mod_file: Option<String>,
    /// The content directory, when it may be read.
    pub content_root: Option<PathBuf>,
    /// Whether the content may be walked.
    pub readable: bool,
    /// The workshop id, when the descriptor names one.
    pub remote_file_id: Option<String>,
    /// How it was found: `mod_file`, `workshop_dir`, `unregistered_dir`, `loose_archive` or
    /// `launcher_db_only`.
    pub origin: &'static str,
}

/// Where everything is, and every mod there.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocateOutput {
    /// The directories and how each was found.
    pub located: Located,
    /// Every mod discovered, in key order.
    pub mods: Vec<LocatedMod>,
    /// Location and discovery diagnostics.
    pub diagnostics: Vec<ScanDiagnostic>,
    /// Discovery may have missed mods: a walk bound tripped, the user directory is
    /// unresolved, or a Steam / unattributed install has no workshop directory. A scan
    /// over these directories should carry this into its own completeness.
    pub incomplete: bool,
    /// The installed game version as `launcher-settings.json` spells it, when readable.
    pub game_version: Option<String>,
}

fn role(r: SourceRole) -> &'static str {
    match r {
        SourceRole::Vanilla => "vanilla",
        SourceRole::Dlc => "dlc",
        SourceRole::Workshop => "workshop",
        SourceRole::Local => "local",
        SourceRole::Archive => "archive",
        _ => "other",
    }
}

fn pattern(p: &VersionPattern) -> String {
    let part = |c: &VersionComponent| match c {
        VersionComponent::Exact(n) => n.to_string(),
        VersionComponent::Wildcard => "*".to_owned(),
    };
    p.0.iter().map(part).collect::<Vec<_>>().join(".")
}

fn located_mod(m: &DiscoveredMod) -> LocatedMod {
    let d = &m.descriptor;
    LocatedMod {
        key: m.key.as_str().to_owned(),
        name: m.name.to_string(),
        version: d.version.as_deref().map(str::to_owned),
        supported_version: d.supported_version.as_ref().map(pattern),
        role: role(m.role),
        mod_file: m.mod_file.as_deref().map(str::to_owned),
        content_root: m.content_root.clone(),
        readable: m.content == ContentState::Readable,
        remote_file_id: d.remote_file_id.as_deref().map(str::to_owned),
        origin: m.origin.as_str(),
    }
}

/// Why the mod list cannot be taken as complete, when a directory that holds mods is
/// unresolved. A GOG or Xbox install has no workshop by design, so it is not flagged.
fn location_gap(l: &Located) -> Option<&'static str> {
    if l.user_dir.is_none() {
        return Some("user directory not found: local mods were not looked for");
    }
    let steamish = matches!(l.store, Store::Steam | Store::Unknown);
    (steamish && l.workshop_dirs.is_empty())
        .then_some("no workshop directory found: workshop mods were not looked for")
}

/// [`locate`] on an injected machine.
///
/// # Errors
/// [`ScanError::Profile`] when the shipped profile does not read.
pub fn locate_with(req: &LocateRequest, env: &dyn HostEnv) -> Result<LocateOutput, ScanError> {
    let profile = Profile::shipped_default().map_err(ScanError::Profile)?;
    let located = smd_ingest::locate(&profile, &req.overrides, env);
    let mut loc = ResolvedLocations::new();
    if let Some(p) = &located.game_dir {
        loc.insert(KEY_INSTALL, p.clone());
    }
    if let Some(p) = &located.user_dir {
        loc.insert(KEY_USER_DIR, p.clone());
    }
    for (n, p) in located.workshop_dirs.iter().enumerate() {
        loc.insert(&workshop_key(n), p.clone());
    }
    let disc = discover(&loc, &DiscoveryOptions { home: req.home.clone() });
    let mut diagnostics: Vec<ScanDiagnostic> =
        located.diagnostics.iter().map(rows::from_ingest).collect();
    diagnostics.extend(disc.diagnostics.iter().map(rows::from_ingest));
    let gv = check_game_version(&loc, &profile);
    diagnostics.extend(gv.diagnostics.iter().map(rows::from_ingest));
    let gap = location_gap(&located);
    if let Some(why) = gap {
        let d = IngestDiagnostic::warn(codes::LOCATE_INCOMPLETE, "", why);
        diagnostics.push(rows::from_ingest(&d));
    }
    Ok(LocateOutput {
        mods: disc.mods.iter().map(located_mod).collect(),
        incomplete: disc.incomplete || gap.is_some(),
        game_version: gv.raw,
        located,
        diagnostics,
    })
}

/// Find the game's directories on this machine and every mod under them.
///
/// # Errors
/// [`ScanError::Profile`] when the shipped profile does not read.
pub fn locate(req: &LocateRequest) -> Result<LocateOutput, ScanError> {
    locate_with(req, &SystemEnv)
}
