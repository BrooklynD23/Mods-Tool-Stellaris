//! The source table: what the engine knows about each mod, and its precedence.

use smallvec::SmallVec;

use crate::ids::{ModKey, SourceId};
use crate::resolution::Confidence;

/// Everything the engine knows about one mod, keyed by its [`SourceId`].
///
/// There is no per-source `enabled` flag and no per-source order confidence: activation is
/// the three-valued [`Activation`], and order and activation confidence are scan-wide in
/// [`PlaysetProvenance`], so there is no copy to disagree with it. (Doc 03, H7.)
///
/// ```compile_fail
/// use smd_core::{Activation, ModKey, SourceMeta, SourceRole};
/// let _ = SourceMeta {
///     key: ModKey("k".into()),
///     name: "n".into(),
///     rank: Vec::new(),
///     activation: Activation::Active,
///     role: SourceRole::Local,
///     deps: Vec::new(),
///     enabled: true,
/// };
/// ```
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceMeta {
    /// Stable across runs and machines.
    pub key: ModKey,
    /// Display name, as the descriptor gives it.
    pub name: Box<str>,
    /// Precedence, compared lexicographically; higher wins. **Empty means the order is
    /// unknown**, not "loads first" — the honest representation of the `DirectoryScan`
    /// fallback.
    ///
    /// A vector rather than an integer so that tiers compare correctly without reserving
    /// numeric ranges (vanilla `[0]`, DLC `[1, n]`, mods `[2, n]`), so a reorder can
    /// insert between two existing sources without renumbering, and so a reorder rewrites
    /// 300 short vectors and nothing else. (REVIEW-2026-08 § G2.)
    pub rank: Vec<u32>,
    /// Whether the playset has it switched on. `DirectoryScan` yields
    /// [`Unknown`](Activation::Unknown) for every source: found on disk is not enabled.
    pub activation: Activation,
    /// Where it came from.
    pub role: SourceRole,
    /// Declared dependencies, matched as far as they could be.
    pub deps: Vec<ResolvedDependency>,
}

impl SourceMeta {
    /// Whether `self` out-ranks `other`.
    ///
    /// An empty `rank` on either side is *no claim*, so the answer is `None` rather than
    /// a guess. Every precedence comparison in the workspace routes through here, which
    /// is the whole reason [`SourceId`] has no `Ord`.
    #[must_use]
    pub fn precedes(&self, other: &SourceMeta) -> Option<std::cmp::Ordering> {
        if self.rank.is_empty() || other.rank.is_empty() {
            return None;
        }
        Some(self.rank.cmp(&other.rank))
    }
}

/// Every source in a scan, indexed by [`SourceId`] (Invariant P1: dense from 0).
///
/// The one place precedence may be compared. `SourceId` has no `Ord`, so `a < b` does not
/// compile and a caller has to come through here — which is the point: identity and
/// precedence are two different facts about a mod, and a reorder rewrites this table's
/// ranks and nothing else.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SourceTable {
    sources: Vec<SourceMeta>,
}

impl SourceTable {
    /// Build from metadata already in discovery order.
    #[must_use]
    pub fn new(sources: Vec<SourceMeta>) -> SourceTable {
        SourceTable { sources }
    }

    /// Metadata for a source, or `None` for an id from a different scan.
    #[must_use]
    pub fn get(&self, id: SourceId) -> Option<&SourceMeta> {
        self.sources.get(id.index() as usize)
    }

    /// Whether `a` out-ranks `b`. `None` when either rank is empty — no claim — or when
    /// either id is not in this table.
    #[must_use]
    pub fn precedes(&self, a: SourceId, b: SourceId) -> Option<std::cmp::Ordering> {
        self.get(a)?.precedes(self.get(b)?)
    }

    /// How many sources the scan discovered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether the scan discovered nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Every source with its id, in discovery order.
    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &SourceMeta)> {
        self.sources
            .iter()
            .enumerate()
            .map(|(i, m)| (SourceId(u32::try_from(i).unwrap_or(u32::MAX)), m))
    }
}

/// Whether a source is in the enabled set. Independent of the order's confidence: an order
/// can be exact while activation is not, and vice versa.
///
/// `Inactive` sources are inventoried but contribute to no resolution; a contender whose
/// activation is `Unknown` is never a stated winner.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum Activation {
    /// In the enabled set.
    Active,
    /// Present but switched off.
    Inactive,
    /// Not established. The honest default.
    #[default]
    Unknown,
}

/// How a launcher database was read.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[non_exhaustive]
pub enum SnapshotMethod {
    /// The SQLite online-backup API.
    OnlineBackup,
    /// A stable copy of the database files.
    StableFileCopy,
    /// The order source reads no snapshot.
    #[default]
    NotApplicable,
}

/// Scan-wide provenance of the load order, from the `LoadOrderSource` chain (doc 05).
///
/// The only home of order and activation confidence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PlaysetProvenance {
    /// The registry key of the source that answered.
    pub order_source: &'static str,
    /// How far [`SourceMeta::rank`] is trusted.
    pub order: Confidence,
    /// How far the enabled set is trusted.
    pub activation: Confidence,
    /// How the underlying data was read.
    pub snapshot: SnapshotMethod,
}

/// Where a source came from.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum SourceRole {
    /// The base game install.
    Vanilla,
    /// Official downloadable content.
    Dlc,
    /// A Steam Workshop subscription.
    Workshop,
    /// A directory under the user mod folder.
    Local,
    /// A zipped mod read through the VFS.
    Archive,
}

/// A dependency declared by a descriptor, matched against the playset as far as it could
/// be.
#[derive(Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ResolvedDependency {
    /// Matched to a source in the playset.
    Resolved {
        /// The text the descriptor declared.
        declared: Box<str>,
        /// What it resolved to.
        source: SourceId,
    },
    /// Declared by display name, so matching is fuzzy. The `u8` is a 0–100 match score;
    /// candidates are shown, never auto-applied.
    Missing {
        /// The text the descriptor declared.
        declared: Box<str>,
        /// Plausible matches, best first.
        candidates: SmallVec<[(SourceId, u8); 2]>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn meta(rank: Vec<u32>) -> SourceMeta {
        SourceMeta {
            key: ModKey("k".into()),
            name: "n".into(),
            rank,
            activation: Activation::Active,
            role: SourceRole::Workshop,
            deps: Vec::new(),
        }
    }

    #[test]
    fn rank_compares_lexicographically_by_tier() {
        assert_eq!(meta(vec![2, 5]).precedes(&meta(vec![1, 99])), Some(Ordering::Greater));
        assert_eq!(meta(vec![0]).precedes(&meta(vec![1, 0])), Some(Ordering::Less));
        assert_eq!(meta(vec![2, 3]).precedes(&meta(vec![2, 3])), Some(Ordering::Equal));
    }

    #[test]
    fn an_empty_rank_is_no_claim_rather_than_first() {
        assert_eq!(meta(vec![]).precedes(&meta(vec![2, 0])), None);
        assert_eq!(meta(vec![2, 0]).precedes(&meta(vec![])), None);
    }

    #[test]
    fn the_table_is_the_only_route_to_a_precedence_comparison() {
        let table = SourceTable::new(vec![meta(vec![0]), meta(vec![2, 1]), meta(vec![])]);
        assert_eq!(table.len(), 3);
        assert!(!table.is_empty());
        assert_eq!(table.precedes(SourceId(1), SourceId(0)), Some(Ordering::Greater));
        assert_eq!(table.precedes(SourceId(0), SourceId(1)), Some(Ordering::Less));
        // Empty rank on either side, or an id from another scan: no claim, not a guess.
        assert_eq!(table.precedes(SourceId(0), SourceId(2)), None);
        assert_eq!(table.precedes(SourceId(0), SourceId(99)), None);
        assert_eq!(table.get(SourceId(99)), None);

        let ids: Vec<u32> = table.iter().map(|(id, _)| id.index()).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }
}
