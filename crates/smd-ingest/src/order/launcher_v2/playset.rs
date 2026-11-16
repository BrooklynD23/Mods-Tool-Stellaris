//! Turning verified launcher rows into a [`Playset`] keyed by [`ModKey`].

use std::path::PathBuf;

use smd_core::{Confidence, ModKey, Playset, PlaysetEntry, SnapshotMethod};

use super::rows::{ModRow, Rows};
use crate::DiscoveredMod;
use crate::codes::{self, IngestDiagnostic};

/// The discovered mod a launcher `mods` row refers to: by workshop id, else by directory.
fn find<'a>(row: &ModRow, mods: &'a [DiscoveredMod]) -> Option<&'a DiscoveredMod> {
    if let Some(steam) = row.steam_id.as_deref() {
        let by_id = mods.iter().find(|m| m.descriptor.remote_file_id.as_deref() == Some(steam));
        if by_id.is_some() {
            return by_id;
        }
    }
    let dir = row.dir_path.as_deref()?;
    let canon = std::fs::canonicalize(dir).unwrap_or_else(|_| PathBuf::from(dir));
    mods.iter().find(|m| m.content_root.as_deref() == Some(canon.as_path()))
}

/// Build the playset. An entry that matches no discovered mod is dropped with a
/// diagnostic; positions become the ordinal among the entries kept.
pub(super) fn build(
    rows: &Rows,
    mods: &[DiscoveredMod],
    method: SnapshotMethod,
    diags: &mut Vec<IngestDiagnostic>,
) -> Playset {
    let mut entries: Vec<PlaysetEntry> = Vec::new();
    for e in &rows.entries {
        let row = rows.mods.iter().find(|m| m.id == e.mod_id);
        let Some(found) = row.and_then(|r| find(r, mods)) else {
            let label = row.and_then(|r| r.display_name.as_deref()).unwrap_or(&e.mod_id);
            let why = "in the launcher playset but not found on disk";
            diags.push(IngestDiagnostic::warn(codes::PLAYSET_MOD_UNMATCHED, label, why));
            continue;
        };
        let key: ModKey = found.key.clone();
        if entries.iter().any(|x| x.key == key) {
            continue;
        }
        let position = u32::try_from(entries.len()).unwrap_or(u32::MAX);
        entries.push(PlaysetEntry { key, position, enabled: e.enabled });
    }
    Playset {
        name: rows.playset_name.clone().into(),
        entries,
        order: Confidence::Exact,
        activation: Confidence::Exact,
        snapshot: method,
    }
}
