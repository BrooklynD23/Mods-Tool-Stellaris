//! `SymbolRules` — the profile-derived inputs Stages C and D need, handed in as data so
//! that this crate never reads a profile and never names a directory.

use std::sync::Arc;

use smd_core::{DomainId, FileOrder, MergeMode};
use smd_profiles::KeyReadings;

/// The key-layer classification of one path (doc 07 `PathClass` + the `DirRules` this
/// milestone reads).
///
/// The merge scope is the domain: entries sharing a domain compete for the same keys.
/// ponytail: no separate `scope` key yet; add `MergeScopeId` when two domains must share a
/// scope or one domain must split.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct PathClass {
    /// Which symbol domain the path's definitions belong to.
    pub domain: Option<DomainId>,
    /// Cross-file behaviour (Stage C).
    pub merge: MergeMode,
    /// Filename direction (Stage D); `None` = unknown, enumerated.
    pub order: Option<FileOrder>,
}

type Classifier = dyn Fn(&str) -> PathClass + Send + Sync;

/// Per-path classification, the `[load_order]` Stage D readings, and domain names.
#[derive(Clone)]
pub struct SymbolRules {
    classify: Arc<Classifier>,
    pub(crate) readings: KeyReadings,
    pub(crate) domains: Arc<[Box<str>]>,
}

impl std::fmt::Debug for SymbolRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymbolRules")
            .field("readings", &self.readings)
            .field("domains", &self.domains.len())
            .finish_non_exhaustive()
    }
}

impl Default for SymbolRules {
    /// Every path unclassified: no key-level outcome is ever stated.
    fn default() -> Self {
        SymbolRules::new(|_| PathClass::default(), KeyReadings::default(), Vec::new())
    }
}

impl SymbolRules {
    /// `classify` maps a normalised path to its class; `domains[i]` names `DomainId(i)`.
    pub fn new(
        classify: impl Fn(&str) -> PathClass + Send + Sync + 'static,
        readings: KeyReadings,
        domains: Vec<Box<str>>,
    ) -> SymbolRules {
        SymbolRules { classify: Arc::new(classify), readings, domains: domains.into() }
    }

    /// The class of a normalised path.
    #[must_use]
    pub fn class_of(&self, path: &str) -> PathClass {
        (self.classify)(path)
    }
}
