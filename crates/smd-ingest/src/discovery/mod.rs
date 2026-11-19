//! Discovery over explicitly given locations (doc 05).
//!
//! Locations are keyed by the names a profile's `[locations]` table declares:
//! [`KEY_INSTALL`], [`KEY_USER_DIR`] and [`KEY_WORKSHOP`]. Nothing here guesses a
//! location; OS-default and Steam-library discovery are the resolver's job.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use smd_core::{Descriptor, Diagnostic, ModKey, ResolvedLocations, SourceRole};

use crate::codes::{self, IngestDiagnostic};
use crate::order::LauncherV2Sqlite;
use crate::order::launcher_v2::LauncherRead;

mod candidate;
mod keys;
mod launcher_db;
mod listing;
mod origin;
mod refuse;
mod unregistered;
mod workshop;

pub(crate) use candidate::UNASSIGNED;
pub(crate) use keys::VANILLA_KEY;
pub use origin::ModOrigin;

/// The game install directory.
pub const KEY_INSTALL: &str = "install";
/// Where vanilla content is read from, when the profile puts it below the install root
/// (`[locations] content_subdir`). Absent = [`KEY_INSTALL`].
pub const KEY_VANILLA_CONTENT: &str = "vanilla_content";
/// The user data directory, holding `mod/`, `dlc_load.json` and `launcher-v2.sqlite`.
pub const KEY_USER_DIR: &str = "user_dir";
/// A workshop content directory (one subdirectory per subscribed mod). A second and later
/// root is keyed [`workshop_key`]`(n)`, so one `ResolvedLocations` can carry any number.
pub const KEY_WORKSHOP: &str = "workshop";

/// The location key of the `n`th workshop root: `workshop`, `workshop.1`, `workshop.2`...
#[must_use]
pub fn workshop_key(n: usize) -> String {
    if n == 0 { KEY_WORKSHOP.to_owned() } else { format!("{KEY_WORKSHOP}.{n}") }
}

fn is_workshop_key(key: &str) -> bool {
    key == KEY_WORKSHOP || key.strip_prefix("workshop.").is_some_and(|n| n.parse::<usize>().is_ok())
}
/// The launcher-descriptor subdirectory of the user directory.
pub const MODS_SUBDIR: &str = "mod";

/// Knobs that are not locations.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DiscoveryOptions {
    /// The home directory descriptors may not swallow; `None` reads `$HOME` (or
    /// `%USERPROFILE%`). Tests pin it.
    pub home: Option<PathBuf>,
}

impl DiscoveryOptions {
    fn home_dir(&self) -> Option<PathBuf> {
        let from_env = || std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        self.home
            .clone()
            .or_else(|| from_env().map(PathBuf::from))
            .filter(|p| !p.as_os_str().is_empty())
    }
}

/// Why a mod's content was not read.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum UnreadReason {
    /// The descriptor is over a cap (`ingest.descriptor_invalid`).
    DescriptorInvalid,
    /// The descriptor file could not be read.
    DescriptorUnreadable,
    /// `path=`/`archive=` was refused (`ingest.root_refused`).
    RootRefused,
    /// `path=` names a directory that is not there.
    PathMissing,
    /// The descriptor names neither a path nor an archive.
    NoPath,
    /// An `archive=` mod; archive reading is not open yet (H23, inventory form).
    Archive,
}

/// Whether a mod's content directory may be walked.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ContentState {
    /// `content_root` is a resolved, permitted directory.
    Readable,
    /// Inventoried only; the content was not and must not be read.
    Unread(UnreadReason),
}

/// One mod found on disk.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DiscoveredMod {
    /// Stable identity.
    pub key: ModKey,
    /// Display name; the directory name when the descriptor gave none or was rejected.
    pub name: Box<str>,
    /// The launcher file's stem or the workshop directory name.
    pub dir_name: Box<str>,
    /// The launcher `*.mod` file name, when the mod came from one.
    pub mod_file: Option<Box<str>>,
    /// The descriptor as read (default when rejected).
    pub descriptor: Descriptor,
    /// Descriptor diagnostics; their `Site::source` is set when ids are assigned.
    pub diagnostics: Vec<Diagnostic>,
    /// Where it came from.
    pub role: SourceRole,
    /// The canonical content directory, `Some` only when `content` is `Readable`.
    pub content_root: Option<PathBuf>,
    /// Whether the content may be walked.
    pub content: ContentState,
    /// How it was found; says whether the game will load it.
    pub origin: ModOrigin,
    /// The resolved `path=`/`archive=` (or directory/file) the mod stands for, readable or
    /// not. Other sources are compared against it to tell "already found".
    pub source_path: Option<PathBuf>,
}

/// Everything discovery found, in `ModKey` byte order.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Discovery {
    /// The game install directory, when one was given and exists.
    pub install: Option<PathBuf>,
    /// Mods sorted by key bytes, never by enumeration order.
    pub mods: Vec<DiscoveredMod>,
    /// Scan-wide diagnostics.
    pub diagnostics: Vec<IngestDiagnostic>,
    /// Discovery may have missed mods: a given location was absent or unlistable, or a
    /// link, special file or bound kept an entry out. Absence from `mods` is then not
    /// "not installed".
    pub incomplete: bool,
    /// The scan's one read of the launcher database, shared with the order chain.
    pub(crate) launcher: Option<Arc<LauncherRead>>,
}

fn existing(loc: &ResolvedLocations, key: &str, sink: &mut candidate::Sink) -> Option<PathBuf> {
    let given = loc.get(key)?;
    if given.is_dir() {
        return Some(given.to_path_buf());
    }
    let why = format!("{} does not exist or is not a directory", given.display());
    sink.diags.push(IngestDiagnostic::warn(codes::LOCATION_MISSING, key, why));
    // The mods live under the user directory and the workshop root; a missing one means
    // they were not looked at. A missing install says nothing about the mods.
    sink.incomplete |= key != KEY_INSTALL && key != KEY_VANILLA_CONTENT;
    None
}

/// Whether a source discovery could not read may be the workshop directory `ws/<id>`,
/// by the id it names or by the `ugc_<id>.mod` convention.
fn workshop_claims(raw: &candidate::Raw, ws: &Path) -> Vec<PathBuf> {
    let mut ids: Vec<String> = Vec::new();
    // A Paradox Mods id is not a workshop directory name.
    if raw.role != SourceRole::ParadoxMods {
        ids.extend(raw.descriptor.remote_file_id.as_deref().map(str::to_owned));
    }
    if let Some(id) = raw.dir_name.strip_prefix("ugc_") {
        ids.push(id.to_owned());
    }
    ids.into_iter()
        .filter(|i| !i.is_empty() && !i.contains(['/', '\\']))
        .map(|i| ws.join(i))
        .collect()
}

/// Discover every mod under the given locations.
#[must_use]
pub fn discover(loc: &ResolvedLocations, opts: &DiscoveryOptions) -> Discovery {
    discover_with_launcher(loc, opts, &LauncherV2Sqlite::default())
}

/// As [`discover`], reading the launcher database through `launcher` (its options and test
/// hooks). That one read is kept on the result and reused by the order chain.
#[must_use]
pub fn discover_with_launcher(
    loc: &ResolvedLocations,
    opts: &DiscoveryOptions,
    launcher: &LauncherV2Sqlite,
) -> Discovery {
    let mut launcher_read = None;
    let mut sink = candidate::Sink::default();
    let vanilla_key =
        if loc.get(KEY_VANILLA_CONTENT).is_some() { KEY_VANILLA_CONTENT } else { KEY_INSTALL };
    let install = existing(loc, vanilla_key, &mut sink);
    let user_dir = existing(loc, KEY_USER_DIR, &mut sink);
    let mut workshops: Vec<PathBuf> = Vec::new();
    for (key, _) in loc.iter().filter(|(k, _)| is_workshop_key(k)) {
        let found = existing(loc, key, &mut sink);
        // The same directory given twice (two spellings, two flags) is read once.
        let dup = |w: &PathBuf, f: &PathBuf| smd_fs::same_host_dir(w, f);
        workshops.extend(found.filter(|f| !workshops.iter().any(|w| dup(w, f))));
    }
    let mut protected: Vec<PathBuf> = opts.home_dir().into_iter().collect();
    protected.extend(loc.iter().map(|(_, p)| p.to_path_buf()));
    protected.extend(user_dir.iter().map(|u| u.join(MODS_SUBDIR)));
    let guard = refuse::Guard::new(protected);
    let mut raws = Vec::new();
    if let Some(user) = &user_dir {
        raws = candidate::user_mods(&candidate::Ctx { guard: &guard, user_dir: user }, &mut sink);
    }
    for ws in &workshops {
        let ws_canon = std::fs::canonicalize(ws).unwrap_or_else(|_| ws.clone());
        // A launcher file that was rejected or refused still names a workshop directory
        // (or its `ugc_<id>` name does); that directory is the same mod, not another.
        // Claims compare by host path identity (`smd_fs::same_host_dir`), not bytes: the
        // launcher writes `D:/Steam/..` where Steam's registry gives `d:/steam/..`.
        let mut claimed: Vec<PathBuf> =
            raws.iter().filter_map(|r| r.content_root.clone()).collect();
        for r in raws.iter().filter(|r| r.content_root.is_none()) {
            claimed.extend(workshop_claims(r, &ws_canon));
        }
        raws.extend(workshop::workshop_mods(ws, &claimed, &smd_fs::same_host_dir, &mut sink));
    }
    match &user_dir {
        Some(user) => {
            let claimed: Vec<PathBuf> = raws
                .iter()
                .flat_map(|r| r.content_root.iter().chain(&r.source_path).cloned())
                .collect();
            raws.extend(unregistered::unregistered(user, &claimed, &mut sink));
            let read = Arc::new(launcher.read_once(loc));
            raws.extend(launcher_db::db_only_mods(&read, user, &guard, &raws, &mut sink));
            launcher_read = Some(read);
        }
        None => {
            let why = "there is no user directory, so mods only the launcher registers were not looked for";
            let d = IngestDiagnostic::warn(codes::LAUNCHER_DB_NOT_CONSULTED, "", why);
            sink.diags.push(d);
        }
    }
    let candidate::Sink { mut diags, incomplete } = sink;
    let mods = finish(raws, &mut diags);
    Discovery { install, mods, diagnostics: diags, incomplete, launcher: launcher_read }
}

/// Key every candidate (in the fixed order they were found), then sort by key bytes.
fn finish(raws: Vec<candidate::Raw>, diags: &mut Vec<IngestDiagnostic>) -> Vec<DiscoveredMod> {
    let mut claims = keys::Claims::new();
    let mut out: Vec<DiscoveredMod> = Vec::with_capacity(raws.len());
    for r in raws {
        let trusted = r.content != ContentState::Unread(UnreadReason::DescriptorInvalid)
            && r.content != ContentState::Unread(UnreadReason::DescriptorUnreadable);
        let derived = keys::derive(r.ids.as_ref(), &r.descriptor, &r.dir_name, trusted);
        let (key, dup) = claims.claim(derived, &r.descriptor, &r.dir_name);
        diags.extend(dup);
        let name: Box<str> = if r.descriptor.name.is_empty() {
            r.dir_name.clone()
        } else {
            r.descriptor.name.clone()
        };
        out.push(DiscoveredMod {
            key,
            name,
            dir_name: r.dir_name,
            mod_file: r.mod_file,
            descriptor: r.descriptor,
            diagnostics: r.diagnostics,
            role: r.role,
            content_root: r.content_root,
            content: r.content,
            origin: r.origin,
            source_path: r.source_path,
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_identity;
#[cfg(test)]
mod tests_sources;
