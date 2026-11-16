//! `ScanIndex` — the immutable, queryable model of "what the game will actually see".
//!
//! This milestone holds the file layer: sources, interners, the overlay, and Stage A/B.
//! The symbol layer (definitions, localisation, events), `PathClassTable` and `reorder`
//! arrive with Stages C and D.

use std::sync::Arc;

use smd_core::{Diagnostic, PathId, PlaysetProvenance, Resolution, SamePath, SourceTable};

use crate::derived::Derived;
use crate::interner::Interner;
use crate::overlay::OverlayTable;
use crate::overlay_rules::ReplacePathScope;
use crate::replace::ReplaceDecl;
use crate::stage_a::{PathInput, PathVerdict, resolve};

/// Built by [`IndexBuilder`](crate::IndexBuilder), then shared by `&` and never mutated.
#[derive(Clone, Debug)]
pub struct ScanIndex {
    pub(crate) sources: SourceTable,
    pub(crate) provenance: PlaysetProvenance,
    pub(crate) paths: Arc<Interner>,
    pub(crate) symbols: Arc<Interner>,
    pub(crate) overlay: Arc<OverlayTable>,
    pub(crate) same_path: Arc<[SamePath]>,
    pub(crate) decls: Arc<[ReplaceDecl]>,
    pub(crate) scope: ReplacePathScope,
    pub(crate) derived: Derived,
    pub(crate) diags: Arc<Vec<Diagnostic>>,
}

impl ScanIndex {
    /// Sources and their `rank` — the only place load order is compared.
    #[must_use]
    pub fn sources(&self) -> &SourceTable {
        &self.sources
    }

    /// Scan-wide order and activation confidence.
    #[must_use]
    pub fn provenance(&self) -> &PlaysetProvenance {
        &self.provenance
    }

    /// The overlay table.
    #[must_use]
    pub fn overlay(&self) -> &OverlayTable {
        &self.overlay
    }

    /// Stage A/B results.
    #[must_use]
    pub fn derived(&self) -> &Derived {
        &self.derived
    }

    /// Diagnostics the pipeline supplied.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diags
    }

    /// The normalised text of a path; empty for an id from another scan.
    #[must_use]
    pub fn path_text(&self, p: PathId) -> &str {
        self.paths.get(p.0)
    }

    /// The id of a normalised path, if any source ships it.
    #[must_use]
    pub fn path_id(&self, text: &str) -> Option<PathId> {
        self.paths.find(text).map(PathId)
    }

    /// The as-shipped spelling of a contribution's file name.
    #[must_use]
    pub fn raw_name(&self, c: &crate::Contribution) -> &str {
        self.symbols.get(c.raw_name.0)
    }

    /// How many interned paths (including the empty string at id 0).
    #[must_use]
    pub fn path_count(&self) -> u32 {
        self.paths.len()
    }

    /// The directory's Stage A behaviour for `p`.
    #[must_use]
    pub fn same_path_of(&self, p: PathId) -> SamePath {
        self.same_path.get(p.0 as usize).copied().unwrap_or_default()
    }

    /// The profile's `replace_path_scope`.
    #[must_use]
    pub fn replace_path_scope(&self) -> ReplacePathScope {
        self.scope
    }

    /// Stage A with its participants and shadow causes.
    #[must_use]
    pub fn path_verdict(&self, p: PathId) -> PathVerdict {
        resolve(&PathInput {
            sources: &self.sources,
            path_id: p,
            path: self.path_text(p),
            contribs: self.overlay.contributions(p),
            same_path: self.same_path_of(p),
            decls: &self.decls,
            scope: self.scope,
        })
    }

    /// Stage A only (doc 07): complete on its own.
    #[must_use]
    pub fn resolve_path(&self, p: PathId) -> Resolution {
        self.path_verdict(p).resolution
    }
}
