//! `DirectoryScan`: every mod found, no order, no claim about activation.

use smd_core::{
    Confidence, LoadOrderSource, OrderError, Playset, PlaysetEntry, ResolvedLocations,
    SnapshotMethod,
};

use super::{ChainSource, SourceAttempt};
use crate::{Discovery, DiscoveryOptions, discover};

/// Registry key.
pub const ID: &str = "directory_scan";

/// The last resort. `rank` ends up empty and `activation` `Unknown` for every mod: found
/// on disk is not enabled, so no source it describes is ever `Active`.
#[derive(Clone, Debug, Default)]
pub struct DirectoryScan {
    discovery: DiscoveryOptions,
}

impl DirectoryScan {
    /// A scan using these discovery options.
    #[must_use]
    pub fn new(discovery: DiscoveryOptions) -> DirectoryScan {
        DirectoryScan { discovery }
    }
}

impl LoadOrderSource for DirectoryScan {
    fn id(&self) -> &'static str {
        ID
    }

    fn ceiling(&self) -> (Confidence, Confidence) {
        (Confidence::Fallback, Confidence::Fallback)
    }

    fn try_load(&self, loc: &ResolvedLocations) -> Result<Playset, OrderError> {
        self.load(loc).result
    }
}

impl ChainSource for DirectoryScan {
    fn load(&self, loc: &ResolvedLocations) -> SourceAttempt {
        self.load_with(loc, &discover(loc, &self.discovery))
    }

    fn load_with(&self, _loc: &ResolvedLocations, found: &Discovery) -> SourceAttempt {
        // `enabled` is a placeholder under `Fallback`; ingest maps every one to `Unknown`.
        let entries = (0u32..)
            .zip(&found.mods)
            .map(|(position, m)| PlaysetEntry { key: m.key.clone(), position, enabled: false })
            .collect();
        let playset = Playset {
            name: "".into(),
            entries,
            order: Confidence::Fallback,
            activation: Confidence::Fallback,
            snapshot: SnapshotMethod::NotApplicable,
        };
        // Discovery's own diagnostics are reported once, by the inventory.
        SourceAttempt { result: Ok(playset), diagnostics: Vec::new() }
    }
}
