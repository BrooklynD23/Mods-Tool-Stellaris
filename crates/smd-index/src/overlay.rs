//! `OverlayTable` — the file layer: `PathId` -> contributors, in discovery order.

use smallvec::SmallVec;
use smd_core::{Blob, ParseCompleteness, PathId, SourceId, SymbolId};

/// One source's copy of one path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Contribution {
    /// Who ships it.
    pub source: SourceId,
    /// Content address. `None` when the bytes were never established (unreadable, or
    /// changed during the scan), so agreement can never be claimed over it.
    pub blob: Option<Blob>,
    /// As-shipped spelling: the filename sort key. Interned in the symbol interner.
    pub raw_name: SymbolId,
    /// How completely the file was read.
    pub completeness: ParseCompleteness,
    /// The source ships two entries normalising to this path (`IdentityCollision`).
    pub ambiguous: bool,
    /// Index into the derived bitsets. Dense from 0, in `(PathId, SourceId)` order;
    /// definition, loc and event sites take later slots from the same counter.
    pub slot: u32,
}

/// Indexed by `PathId`. Built once in discovery order, so it does not depend on load order.
#[derive(Clone, Debug, Default)]
pub struct OverlayTable {
    by_path: Vec<SmallVec<[Contribution; 2]>>,
    slots: u32,
}

impl OverlayTable {
    pub(crate) fn new(by_path: Vec<SmallVec<[Contribution; 2]>>, slots: u32) -> OverlayTable {
        OverlayTable { by_path, slots }
    }

    /// Contributors of `p`, ascending by `SourceId::index()`.
    #[must_use]
    pub fn contributions(&self, p: PathId) -> &[Contribution] {
        self.by_path.get(p.0 as usize).map_or(&[], |v| v.as_slice())
    }

    /// How many contribution slots exist.
    #[must_use]
    pub fn slot_count(&self) -> u32 {
        self.slots
    }

    /// Paths with at least two contributors, ascending.
    pub fn multi_contributor_paths(&self) -> impl Iterator<Item = PathId> + '_ {
        self.by_path
            .iter()
            .enumerate()
            .filter(|(_, v)| v.len() >= 2)
            .map(|(i, _)| PathId(u32::try_from(i).unwrap_or(u32::MAX)))
    }

    /// Every path with at least one contributor, ascending.
    pub fn paths(&self) -> impl Iterator<Item = PathId> + '_ {
        self.by_path
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.is_empty())
            .map(|(i, _)| PathId(u32::try_from(i).unwrap_or(u32::MAX)))
    }

    /// The slot of `source`'s copy of `p`.
    #[must_use]
    pub fn slot_of(&self, p: PathId, source: SourceId) -> Option<u32> {
        self.contributions(p).iter().find(|c| c.source == source).map(|c| c.slot)
    }
}
