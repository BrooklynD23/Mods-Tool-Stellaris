//! Mods sitting in `<user_dir>/mod` that no launcher `*.mod` file points at: directories
//! copied in by hand, and loose archives.

use std::path::{Path, PathBuf};

use smd_core::{DescriptorSource, SourceRole};
use smd_fs::ReadOnlyRoot;

use super::candidate::{Raw, Sink, unread};
use super::listing::{self, Entry, Kind};
use super::workshop::dir_mod;
use super::{ContentState, ModOrigin, UnreadReason};
use crate::codes::{self, IngestDiagnostic};

fn is_zip(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".zip")
}

fn is_claimed(claimed: &[PathBuf], here: &Path) -> bool {
    claimed.iter().any(|c| smd_fs::same_host_dir(c, here))
}

/// Directories with a `descriptor.mod`, and `*.zip` files, directly under `<user>/mod` that
/// none of `claimed` (every path an already-found mod stands for) names.
pub(super) fn unregistered(user_dir: &Path, claimed: &[PathBuf], sink: &mut Sink) -> Vec<Raw> {
    let dir = user_dir.join(super::MODS_SUBDIR);
    // A `mod/` that cannot be opened was already reported by the `*.mod` pass.
    let Ok(root) = ReadOnlyRoot::open(&dir) else { return Vec::new() };
    // `*.mod` entries were listed (and their gaps reported) by that pass; only what it did
    // not care about is new here, but a repeat of a gap it did report is not.
    let mut fresh = Vec::new();
    let listed =
        listing::list(root.path(), super::MODS_SUBDIR, |n| !n.ends_with(".mod"), &mut fresh);
    let new: Vec<_> = fresh.into_iter().filter(|d| !sink.diags.contains(d)).collect();
    sink.diags.extend(new);
    sink.incomplete |= listed.incomplete;
    let mut out = Vec::new();
    for e in &listed.entries {
        let here = root.path().join(&e.name);
        if is_claimed(claimed, &here) {
            continue;
        }
        match e.kind {
            Kind::Dir => out.extend(one_dir(&root, e, here, sink)),
            Kind::File if is_zip(&e.text()) => out.push(one_zip(e, here)),
            Kind::File => {}
        }
    }
    out
}

fn one_dir(root: &ReadOnlyRoot, e: &Entry, here: PathBuf, sink: &mut Sink) -> Option<Raw> {
    let name = e.text();
    let label = format!("{}/{name}", super::MODS_SUBDIR);
    let mut raw = dir_mod(root, std::slice::from_ref(&e.name), &name, &label, sink)?;
    raw.role = SourceRole::Local;
    raw.origin = ModOrigin::UnregisteredDir;
    raw.source_path = Some(here);
    if raw.content == ContentState::Readable {
        raw.descriptor.source = DescriptorSource::Local;
    }
    let why = "has a descriptor.mod but no launcher .mod file points at it; the launcher does \
               not know it, so the game does not load it";
    sink.diags.push(IngestDiagnostic::warn(codes::MOD_UNREGISTERED, &label, why));
    Some(raw)
}

fn one_zip(e: &Entry, here: PathBuf) -> Raw {
    let name = e.text();
    let stem = name.get(..name.len() - ".zip".len()).unwrap_or(&name);
    let mut raw = unread(stem, None, UnreadReason::Archive);
    raw.role = SourceRole::Archive;
    raw.origin = ModOrigin::LooseArchive;
    raw.source_path = Some(here);
    raw
}
