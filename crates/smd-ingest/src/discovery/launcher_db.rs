//! Mods only the launcher database knows: `mods` rows no other source already found
//! (Paradox Mods downloads, mods registered from arbitrary folders).
//!
//! The rows come from the scan's one launcher read (a verified read-only snapshot); the
//! live file is never opened here. A row's path is untrusted exactly as a descriptor's `path=` is: it
//! goes through the same refuse [`Guard`].

use std::path::Path;

use smd_core::{DescriptorSource, SourceRole};
use smd_fs::ReadOnlyRoot;

use super::candidate::{Raw, Sink, unread};
use super::keys::Provider;
use super::refuse::Guard;
use super::workshop::dir_mod;
use super::{ContentState, ModOrigin, UnreadReason};
use crate::codes::{self, IngestDiagnostic};
use crate::order::launcher_v2::db_mods::DbMods;
use crate::order::launcher_v2::join::{self, Ident};
use crate::order::launcher_v2::rows::ModRow;
use crate::order::launcher_v2::{FILE, LauncherRead};

/// Every launcher row not already found, as a candidate. `found` is what the other sources
/// found. Without a readable database the scan goes on, and says it did not look.
pub(super) fn db_only_mods(
    read: &LauncherRead,
    user_dir: &Path,
    guard: &Guard,
    found: &[Raw],
    sink: &mut Sink,
) -> Vec<Raw> {
    let rows = match read.db_mods() {
        DbMods::Rows(rows) => rows,
        DbMods::NotConsulted(why) => {
            let why = format!("{why}; mods only the launcher registers were not looked for");
            sink.diags.push(IngestDiagnostic::warn(codes::LAUNCHER_DB_NOT_CONSULTED, FILE, why));
            return Vec::new();
        }
    };
    let mut idents: Vec<Ident> = found.iter().map(ident).collect();
    let mut out: Vec<Raw> = Vec::new();
    for row in rows {
        if join::find(row, &idents, user_dir).is_some() {
            continue;
        }
        let Some(raw) = one_row(row, user_dir, guard, sink) else { continue };
        // The row's directory may resolve to a content root already found (a link, another
        // spelling): a second source with the same files would conflict with itself.
        let id = ident(&raw);
        if idents.iter().any(|i| join::share_path(&id, i)) {
            continue;
        }
        idents.push(id);
        out.push(raw);
    }
    out
}

/// The namespaced id a candidate stands for, as the join compares it.
fn remote_ref(r: &Raw) -> Option<String> {
    let (provider, id) = match &r.ids {
        Some((p, id)) => (*p, id.as_ref()),
        None => (Provider::Steam, r.descriptor.remote_file_id.as_deref()?),
    };
    Some(join::remote(provider == Provider::Paradox, id))
}

fn ident(r: &Raw) -> Ident {
    Ident {
        mod_file: r.mod_file.as_deref().map(str::to_owned),
        remote_id: remote_ref(r),
        evidenced: r.ids.is_some() || join::evidenced_file(r.mod_file.as_deref()),
        paths: r.content_root.iter().chain(&r.source_path).cloned().collect(),
    }
}

/// `None` for a row naming no path at all: there is nothing to inventory.
fn one_row(row: &ModRow, user_dir: &Path, guard: &Guard, sink: &mut Sink) -> Option<Raw> {
    let archive_only = row.dir_path.is_none();
    let declared = row.dir_path.as_deref().or(row.archive_path.as_deref())?;
    let shown = row.display_name.as_deref().unwrap_or(declared);
    let label = format!("{FILE}: {shown}");
    let mut raw = match guard.resolve(user_dir, Path::new(declared)) {
        Err(why) => {
            sink.diags.push(IngestDiagnostic::error(codes::ROOT_REFUSED, &label, why));
            unread(&base_name(declared), None, UnreadReason::RootRefused)
        }
        Ok(path) => {
            let mut raw = read_path(&path, archive_only, &label, sink);
            raw.source_path = Some(path);
            raw
        }
    };
    raw.origin = ModOrigin::LauncherDbOnly;
    // Evidence for the platform, strongest first (see `keys::provider_of_file`): the row's
    // `steamId`, then its `pdxId`, then the descriptor's id read as Steam.
    raw.ids = match (&row.steam_id, &row.pdx_id) {
        (Some(id), _) => Some((Provider::Steam, id.as_str().into())),
        (None, Some(id)) => Some((Provider::Paradox, id.as_str().into())),
        (None, None) => None,
    };
    raw.role = if archive_only {
        SourceRole::Archive
    } else if matches!(raw.ids, Some((Provider::Paradox, _))) {
        SourceRole::ParadoxMods
    } else if raw.ids.is_some() || raw.descriptor.remote_file_id.is_some() {
        SourceRole::Workshop
    } else {
        SourceRole::Local
    };
    if raw.content == ContentState::Readable {
        raw.descriptor.source = match raw.role {
            SourceRole::Workshop => DescriptorSource::Workshop,
            _ => DescriptorSource::Local,
        };
    }
    Some(raw)
}

fn base_name(declared: &str) -> String {
    let trimmed = declared.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).to_owned()
}

fn read_path(path: &Path, archive_only: bool, label: &str, sink: &mut Sink) -> Raw {
    let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if archive_only {
        return unread(&name, None, UnreadReason::Archive);
    }
    if !path.is_dir() {
        let why = format!("{} is not a directory", path.display());
        sink.diags.push(IngestDiagnostic::warn(codes::MOD_PATH_MISSING, label, why));
        return unread(&name, None, UnreadReason::PathMissing);
    }
    let found =
        ReadOnlyRoot::open(path).ok().and_then(|root| dir_mod(&root, &[], &name, label, sink));
    found.unwrap_or_else(|| {
        let why = "has no readable descriptor.mod; inventoried, content unread";
        sink.diags.push(IngestDiagnostic::warn(codes::DESCRIPTOR_UNREADABLE, label, why));
        unread(&name, None, UnreadReason::DescriptorUnreadable)
    })
}
