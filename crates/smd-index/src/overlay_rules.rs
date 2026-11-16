//! `OverlayRules` — the profile-derived inputs Stage A needs, defined here so that
//! `smd-index` never reads a profile and never names a game directory.

use std::sync::Arc;

use smd_core::SamePath;

/// The profile's `[load_order].replace_path_scope`.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, Default)]
pub enum ReplacePathScope {
    /// Not established; the resolver enumerates both readings below.
    #[default]
    Unknown,
    /// A `replace_path` drops only the base game's files.
    VanillaOnly,
    /// A `replace_path` also drops files of every earlier mod.
    VanillaAndEarlier,
}

type Classifier = dyn Fn(&str) -> SamePath + Send + Sync;

/// Stage A's per-path and per-scan parameters.
#[derive(Clone)]
pub struct OverlayRules {
    scope: ReplacePathScope,
    classify: Arc<Classifier>,
}

impl std::fmt::Debug for OverlayRules {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OverlayRules").field("scope", &self.scope).finish_non_exhaustive()
    }
}

impl OverlayRules {
    /// `classify` maps a normalised path to its directory's `same_path` behaviour.
    pub fn new(
        scope: ReplacePathScope,
        classify: impl Fn(&str) -> SamePath + Send + Sync + 'static,
    ) -> OverlayRules {
        OverlayRules { scope, classify: Arc::new(classify) }
    }

    /// Every path has the same behaviour.
    #[must_use]
    pub fn uniform(scope: ReplacePathScope, same_path: SamePath) -> OverlayRules {
        OverlayRules::new(scope, move |_| same_path)
    }

    /// A prefix table: the longest matching directory prefix decides, else `default`.
    #[must_use]
    pub fn from_table(
        scope: ReplacePathScope,
        default: SamePath,
        table: Vec<(String, SamePath)>,
    ) -> OverlayRules {
        OverlayRules::new(scope, move |path| {
            table
                .iter()
                .filter(|(prefix, _)| crate::replace::covers(prefix, path))
                .max_by_key(|(prefix, _)| prefix.len())
                .map_or(default, |(_, sp)| *sp)
        })
    }

    /// The `replace_path_scope` reading.
    #[must_use]
    pub fn scope(&self) -> ReplacePathScope {
        self.scope
    }

    /// The `same_path` behaviour of a normalised path.
    #[must_use]
    pub fn same_path_of(&self, path: &str) -> SamePath {
        (self.classify)(path)
    }
}
