//! `Derived` — everything the load order decides. Rebuilt on every reorder; nothing else is.

use std::collections::HashMap;

use smallvec::SmallVec;
use smd_core::{Contender, PathId, ResolutionGap, ShadowCause};

/// A fixed-size bitset.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    /// `len` bits, all clear.
    #[must_use]
    pub fn new(len: u32) -> BitSet {
        BitSet { words: vec![0; (len as usize).div_ceil(64)] }
    }

    /// Set a bit; out-of-range is ignored.
    pub fn set(&mut self, i: u32) {
        if let Some(w) = self.words.get_mut(i as usize / 64) {
            *w |= 1 << (i % 64);
        }
    }

    /// Whether a bit is set.
    #[must_use]
    pub fn get(&self, i: u32) -> bool {
        self.words.get(i as usize / 64).is_some_and(|w| w >> (i % 64) & 1 == 1)
    }

    /// Number of set bits.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }

    /// Whether the two sets share a bit.
    #[must_use]
    pub fn intersects(&self, other: &BitSet) -> bool {
        self.words.iter().zip(&other.words).any(|(a, b)| a & b != 0)
    }
}

/// Stage A and B results.
///
/// `surviving` is the stated winner of a path; `possibly` is every participant of a path
/// whose Stage A outcome is `Unknown`. `surviving ∩ possibly = ∅` always. (`winning` is
/// Stage C's and arrives with it.)
#[derive(Clone, Debug, Default)]
pub struct Derived {
    pub(crate) surviving: BitSet,
    pub(crate) possibly: BitSet,
    pub(crate) shadowed: Vec<(Contender, ShadowCause)>,
    pub(crate) path_gaps: HashMap<PathId, SmallVec<[ResolutionGap; 1]>>,
}

impl Derived {
    /// Stated or possible survivor. The predicate collision rules filter on — never
    /// `surviving` alone.
    #[must_use]
    pub fn contends(&self, slot: u32) -> bool {
        self.surviving.get(slot) || self.possibly.get(slot)
    }

    /// Stated to reach the game.
    #[must_use]
    pub fn is_surviving(&self, slot: u32) -> bool {
        self.surviving.get(slot)
    }

    /// May reach the game (a Stage A gap).
    #[must_use]
    pub fn is_possibly(&self, slot: u32) -> bool {
        self.possibly.get(slot)
    }

    /// The surviving bitset.
    #[must_use]
    pub fn surviving(&self) -> &BitSet {
        &self.surviving
    }

    /// The possibly-surviving bitset.
    #[must_use]
    pub fn possibly(&self) -> &BitSet {
        &self.possibly
    }

    /// Stated shadowings, ascending by `(PathId, SourceId::index())`.
    #[must_use]
    pub fn shadowed(&self) -> &[(Contender, ShadowCause)] {
        &self.shadowed
    }

    /// Stage A gaps of `p`; empty when its outcome is stated.
    #[must_use]
    pub fn path_gaps(&self, p: PathId) -> &[ResolutionGap] {
        self.path_gaps.get(&p).map_or(&[], |g| g.as_slice())
    }
}
