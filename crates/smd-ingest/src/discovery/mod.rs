//! Discovery over explicitly given locations (doc 05).
//!
//! Locations are keyed by the names a profile's `[locations]` table declares:
//! [`KEY_INSTALL`], [`KEY_USER_DIR`] and [`KEY_WORKSHOP`]. Nothing here guesses a
//! location; OS-default and Steam-library discovery are the resolver's job.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use smd_core::{Descriptor, Diagnostic, ModKey, ResolvedLocations, SourceRole};

use crate::codes::{self, IngestDiagnostic};

mod candidate;
mod keys;
mod listing;
mod refuse;
mod workshop;

pub(crate) use candidate::UNASSIGNED;
pub(crate) use keys::VANILLA_KEY;

/// The game install directory.
pub const KEY_INSTALL: &str = "install";
/// The user data directory, holding `mod/`, `dlc_load.json` and `launcher-v2.sqlite`.
pub const KEY_USER_DIR: &str = "user_dir";
/// A workshop content directory (one subdirectory per subscribed mod).
pub const KEY_WORKSHOP: &str = "workshop";
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
    sink.incomplete |= key != KEY_INSTALL;
    None
}

/// Whether a source discovery could not read may be the workshop directory `ws/<id>`,
/// by the id it names or by the `ugc_<id>.mod` convention.
fn workshop_claims(raw: &candidate::Raw, ws: &Path) -> Vec<PathBuf> {
    let mut ids: Vec<String> = Vec::new();
    ids.extend(raw.descriptor.remote_file_id.as_deref().map(str::to_owned));
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
    let mut sink = candidate::Sink::default();
    let install = existing(loc, KEY_INSTALL, &mut sink);
    let user_dir = existing(loc, KEY_USER_DIR, &mut sink);
    let workshop = existing(loc, KEY_WORKSHOP, &mut sink);
    let mut protected: Vec<PathBuf> = opts.home_dir().into_iter().collect();
    protected.extend(loc.iter().map(|(_, p)| p.to_path_buf()));
    protected.extend(user_dir.iter().map(|u| u.join(MODS_SUBDIR)));
    let guard = refuse::Guard::new(protected);
    let mut raws = Vec::new();
    if let Some(user) = &user_dir {
        raws = candidate::user_mods(&candidate::Ctx { guard: &guard, user_dir: user }, &mut sink);
    }
    if let Some(ws) = &workshop {
        let ws_canon = std::fs::canonicalize(ws).unwrap_or_else(|_| ws.clone());
        // A launcher file that was rejected or refused still names a workshop directory
        // (or its `ugc_<id>` name does); that directory is the same mod, not another.
        let mut claimed: BTreeSet<PathBuf> =
            raws.iter().filter_map(|r| r.content_root.clone()).collect();
        for r in raws.iter().filter(|r| r.content_root.is_none()) {
            claimed.extend(workshop_claims(r, &ws_canon));
        }
        raws.extend(workshop::workshop_mods(ws, &claimed, &mut sink));
    }
    let candidate::Sink { mut diags, incomplete } = sink;
    let mods = finish(raws, &mut diags);
    Discovery { install, mods, diagnostics: diags, incomplete }
}

/// Key every candidate (in the fixed order they were found), then sort by key bytes.
fn finish(raws: Vec<candidate::Raw>, diags: &mut Vec<IngestDiagnostic>) -> Vec<DiscoveredMod> {
    let mut claims = keys::Claims::new();
    let mut out: Vec<DiscoveredMod> = Vec::with_capacity(raws.len());
    for r in raws {
        let trusted = r.content != ContentState::Unread(UnreadReason::DescriptorInvalid)
            && r.content != ContentState::Unread(UnreadReason::DescriptorUnreadable);
        let derived = keys::derive(&r.descriptor, &r.dir_name, trusted);
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
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

#[cfg(test)]
mod tests;
