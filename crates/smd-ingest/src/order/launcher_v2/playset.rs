//! Turning verified launcher rows into a [`Playset`] keyed by [`ModKey`].

use std::path::Path;

use smd_core::{Confidence, Playset, PlaysetEntry, SnapshotMethod, SourceRole};

use super::join::{self, Ident};
use super::rows::Rows;
use crate::DiscoveredMod;
use crate::codes::{self, IngestDiagnostic};

/// The platform-qualified id of a discovered mod: its key when that is one, else the
/// descriptor's id under the mod's role.
fn remote_ref(m: &DiscoveredMod) -> Option<String> {
    let key = m.key.as_str();
    if key.starts_with("steam:") || key.starts_with("pdx:") {
        return Some(key.to_owned());
    }
    let id = m.descriptor.remote_file_id.as_deref()?;
    Some(join::remote(m.role == SourceRole::ParadoxMods, id))
}

/// How a discovered mod is recognised by the join.
pub(crate) fn ident(m: &DiscoveredMod) -> Ident {
    Ident {
        mod_file: m.mod_file.as_deref().map(str::to_owned),
        remote_id: remote_ref(m),
        paths: m.content_root.iter().chain(&m.source_path).cloned().collect(),
    }
}

/// Build the playset. An entry that matches no discovered mod is counted in
/// [`Playset::unmatched`] and reported; positions become the ordinal among the entries
/// kept. `base` is the user directory, which a relative launcher path is relative to.
pub(super) fn build(
    rows: &Rows,
    mods: &[DiscoveredMod],
    base: &Path,
    method: SnapshotMethod,
    diags: &mut Vec<IngestDiagnostic>,
) -> Playset {
    let idents: Vec<Ident> = mods.iter().map(ident).collect();
    let mut entries: Vec<PlaysetEntry> = Vec::new();
    let mut unmatched = 0u32;
    for e in &rows.entries {
        let row = rows.mods.iter().find(|m| m.id == e.mod_id);
        let found = row.and_then(|r| join::find(r, &idents, base)).map(|i| &mods[i]);
        let Some(found) = found else {
            let label = row.and_then(|r| r.display_name.as_deref()).unwrap_or(&e.mod_id);
            let why = "in the launcher playset but not found on disk";
            diags.push(IngestDiagnostic::warn(codes::PLAYSET_MOD_UNMATCHED, label, why));
            unmatched = unmatched.saturating_add(1);
            continue;
        };
        let key = found.key.clone();
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
        unmatched,
    }
}
