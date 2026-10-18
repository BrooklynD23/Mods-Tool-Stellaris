//! `DlcLoadJson`: the last launch's enabled mods, in the order the game loaded them.
//!
//! Reflects the **last launch**, not the launcher's current state (VERIFY V08, assumed),
//! so both certainties are `Inferred`.

use std::collections::BTreeMap;
use std::io::ErrorKind;

use smd_core::{
    Confidence, LoadOrderSource, OrderError, Playset, PlaysetEntry, ResolvedLocations,
    SnapshotMethod,
};
use smd_fs::ReadOnlyRoot;

use super::json_caps::{self, FILE_MAX};
use super::{ChainSource, SourceAttempt};
use crate::codes::{self, IngestDiagnostic};
use crate::{DiscoveredMod, Discovery, DiscoveryOptions, KEY_USER_DIR, discover};

/// Registry key.
pub const ID: &str = "dlc_load_json";
/// The file name, beside the launcher database.
pub const FILE: &str = "dlc_load.json";

/// Reads `<user_dir>/dlc_load.json`.
#[derive(Clone, Debug, Default)]
pub struct DlcLoadJson {
    discovery: DiscoveryOptions,
}

impl DlcLoadJson {
    /// A reader using these discovery options.
    #[must_use]
    pub fn new(discovery: DiscoveryOptions) -> DlcLoadJson {
        DlcLoadJson { discovery }
    }
}

impl LoadOrderSource for DlcLoadJson {
    fn id(&self) -> &'static str {
        ID
    }

    fn ceiling(&self) -> (Confidence, Confidence) {
        (Confidence::Inferred, Confidence::Inferred)
    }

    fn try_load(&self, loc: &ResolvedLocations) -> Result<Playset, OrderError> {
        self.load(loc).result
    }
}

fn invalid(diags: &mut Vec<IngestDiagnostic>, why: String) -> OrderError {
    diags.push(IngestDiagnostic::error(codes::DLC_LOAD_INVALID, FILE, why.clone()));
    OrderError::Unreadable { detail: why.into() }
}

/// Read, cap-check and parse the file into the list of `enabled_mods` strings.
fn read_enabled(
    loc: &ResolvedLocations,
    diags: &mut Vec<IngestDiagnostic>,
) -> Result<Vec<String>, OrderError> {
    let user = loc.get(KEY_USER_DIR).ok_or(OrderError::NotPresent)?;
    let root = ReadOnlyRoot::open(user).map_err(|_| OrderError::NotPresent)?;
    // Size first, from the directory entry, so over-cap is a typed outcome and not a
    // substring of an I/O error message. The read still enforces the cap.
    match std::fs::symlink_metadata(root.path().join(FILE)) {
        Ok(m) if m.len() > FILE_MAX => {
            return Err(invalid(diags, format!("file is over the {FILE_MAX}-byte cap")));
        }
        _ => {}
    }
    let bytes = match root.read_file(&[FILE.into()], FILE_MAX) {
        Ok(b) => b,
        Err(e) if e.kind() == ErrorKind::NotFound => return Err(OrderError::NotPresent),
        Err(e) => return Err(OrderError::Unreadable { detail: e.to_string().into() }),
    };
    json_caps::check_depth(&bytes).map_err(|why| invalid(diags, why))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| OrderError::Corrupt { detail: e.to_string().into() })?;
    json_caps::check_entries(&value).map_err(|why| invalid(diags, why))?;
    let Some(list) = value.get("enabled_mods").and_then(|v| v.as_array()) else {
        let found = value.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "));
        return Err(OrderError::SchemaUnknown { found: found.unwrap_or_default().into() });
    };
    let mut names = Vec::with_capacity(list.len());
    for (i, v) in list.iter().enumerate() {
        match v.as_str() {
            Some(s) => names.push(s.to_owned()),
            None => {
                let why = format!("enabled_mods[{i}] is not a string and was ignored");
                diags.push(IngestDiagnostic::warn(codes::DLC_LOAD_INVALID, FILE, why));
            }
        }
    }
    Ok(names)
}

/// `mod/ugc_1.mod` and `mod\ugc_1.mod` both name `ugc_1.mod`.
fn file_name(listed: &str) -> &str {
    listed.rsplit(['/', '\\']).next().unwrap_or(listed)
}

fn entries(
    listed: &[String],
    mods: &[DiscoveredMod],
    diags: &mut Vec<IngestDiagnostic>,
) -> Vec<PlaysetEntry> {
    let by_file: BTreeMap<&str, &DiscoveredMod> =
        mods.iter().filter_map(|m| m.mod_file.as_deref().map(|f| (f, m))).collect();
    let by_lower: BTreeMap<String, &DiscoveredMod> =
        by_file.iter().map(|(f, m)| (f.to_ascii_lowercase(), *m)).collect();
    let mut out: Vec<PlaysetEntry> = Vec::new();
    for item in listed {
        let name = file_name(item);
        let found = by_file.get(name).or_else(|| by_lower.get(&name.to_ascii_lowercase()));
        let Some(m) = found else {
            let why = "listed in enabled_mods but no such launcher file was discovered";
            diags.push(IngestDiagnostic::warn(codes::PLAYSET_MOD_UNMATCHED, item, why));
            continue;
        };
        if out.iter().any(|e| e.key == m.key) {
            continue;
        }
        let position = u32::try_from(out.len()).unwrap_or(u32::MAX);
        out.push(PlaysetEntry { key: m.key.clone(), position, enabled: true });
    }
    out
}

impl ChainSource for DlcLoadJson {
    fn load(&self, loc: &ResolvedLocations) -> SourceAttempt {
        self.load_with(loc, &discover(loc, &self.discovery))
    }

    fn load_with(&self, loc: &ResolvedLocations, found: &Discovery) -> SourceAttempt {
        let mut diagnostics = Vec::new();
        let result = read_enabled(loc, &mut diagnostics).map(|listed| {
            // A discovered mod that is not listed is simply absent: with `Inferred`
            // activation, absence means `Inactive`.
            Playset {
                name: "".into(),
                entries: entries(&listed, &found.mods, &mut diagnostics),
                order: Confidence::Inferred,
                activation: Confidence::Inferred,
                snapshot: SnapshotMethod::NotApplicable,
            }
        });
        SourceAttempt { result, diagnostics }
    }
}
