//! Stage C and D for one localisation key (doc 07 § Stage C, § Stage D; H3).
//!
//! The group is every contending, non-`Inactive` entry of `(lang, key)`. `tier` decides
//! first: only the highest tier present competes, and when it holds one entry the
//! mechanism is `ReplaceTier`. Within that tier the definition resolver's Stage D applies
//! under every reading of the unknown parameters. A winner is stated only if it is
//! surviving, `Active`, and beats every incomplete file in its scope at its tier or above:
//! an unparsed localisation file's keys are unknown, not absent, and could hold this key.
//! A group with no live entry is `Unknown`, never `Deleted`.

use std::cmp::Ordering;

use smallvec::SmallVec;
use smd_core::{
    Activation, Contender, FileOrder, LangId, Mechanism, MergeMode, Outcome, Resolution,
    ResolutionGap, SymbolId,
};

use super::LocSite;
use crate::defs::resolve::{Gaps, agreement, flips, stated, unknown};
use crate::defs::stage_d::{Cand, Decider, Readings, compare, top};
use crate::index::ScanIndex;

/// Stage C's answer for one `(lang, key)`, with its participants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LocVerdict {
    /// The resolution.
    pub resolution: Resolution,
    /// Entries whose file contends (stated or possible survivor), not `Inactive`.
    pub live: SmallVec<[LocSite; 2]>,
}

fn contender(d: &LocSite) -> Contender {
    Contender { source: d.site.source, path: d.site.path, slot: Some(d.file_slot) }
}

/// The scope's shared filename direction; `None` (enumerated) when the classes disagree.
fn shared_order(orders: impl Iterator<Item = Option<FileOrder>>) -> Option<FileOrder> {
    let all: SmallVec<[Option<FileOrder>; 2]> = orders.collect();
    all.first().copied().flatten().filter(|o| all.iter().all(|x| *x == Some(*o)))
}

impl ScanIndex {
    /// Stages C and D for one localisation key.
    #[must_use]
    pub fn resolve_loc(&self, lang: LangId, key: SymbolId) -> LocVerdict {
        let live: SmallVec<[LocSite; 2]> = self
            .loc
            .sites(lang, key)
            .iter()
            .filter(|d| self.activation_of(d.site.source) != Activation::Inactive)
            .filter(|d| self.derived.contends(d.file_slot))
            .copied()
            .collect();
        let resolution = self.loc_contest(&live);
        LocVerdict { resolution, live }
    }

    fn loc_contest(&self, live: &[LocSite]) -> Resolution {
        if live.is_empty() {
            return unknown(Gaps::new());
        }
        let classes: SmallVec<[_; 2]> = live.iter().map(|d| self.class_of(d.site.path)).collect();
        let scope = classes[0].domain;
        // ponytail: only `merge_by_key` is resolved for localisation; any other mode, or a
        // group spanning two scopes, reports its participants without an outcome.
        if classes.iter().any(|c| c.merge != MergeMode::MergeByKey || c.domain != scope) {
            return unknown(SmallVec::from_slice(&[ResolutionGap::MergeModeUnknown]));
        }
        let tier = classes.iter().map(|c| c.tier).max().unwrap_or(0);
        let order = shared_order(classes.iter().map(|c| c.order));
        let at_top: SmallVec<[usize; 2]> =
            (0..live.len()).filter(|&i| classes[i].tier == tier).collect();
        let readings = Readings::new(self.readings, order);
        let cands: Vec<Cand<'_>> = at_top
            .iter()
            .map(|&i| self.cand(live[i].site.source, live[i].site.path, Some(live[i].ordinal)))
            .collect();
        let all = readings.all();
        let results: Vec<_> =
            all.iter().map(|ix| top(&self.sources, &cands, readings.at(*ix))).collect();
        let mut gaps = flips(&readings, &all, &results);
        gaps.extend(results.iter().filter_map(|r| r.as_ref().err()).flatten().copied());
        let winner = match results.first() {
            Some(Ok(i)) if results.iter().all(|r| r.as_ref().ok() == Some(i)) => Some(*i),
            _ => None,
        };
        if let Some(i) = winner {
            gaps.extend(self.loc_doubts(&live[at_top[i]], &cands[i], tier, (&readings, &all)));
        }
        gaps.sort();
        gaps.dedup();
        match winner {
            Some(i) if gaps.is_empty() => {
                let mechanism = if cands.len() == 1 && live.len() > 1 {
                    Mechanism::ReplaceTier
                } else {
                    let first = readings.at(all[0]);
                    let by_rank = (0..cands.len()).filter(|&j| j != i).any(|j| {
                        matches!(
                            compare(&self.sources, &cands[i], &cands[j], first),
                            Ok((_, Decider::Rank))
                        )
                    });
                    if by_rank { Mechanism::LoadOrder } else { Mechanism::FilenameOrder }
                };
                self.loc_stated(live, at_top[i], mechanism)
            }
            _ => unknown(gaps),
        }
    }

    fn loc_stated(&self, live: &[LocSite], w: usize, mechanism: Mechanism) -> Resolution {
        let hashes: SmallVec<[[u8; 16]; 2]> = live.iter().map(|d| d.value_hash).collect();
        let vanilla: Vec<bool> = live.iter().map(|d| self.is_vanilla(d.site.source)).collect();
        let mut r = stated(Outcome::SingleWinner, mechanism, agreement(&hashes, Some(w), &vanilla));
        r.winner = Some(contender(&live[w]));
        r.discarded =
            live.iter().enumerate().filter(|(j, _)| *j != w).map(|(_, d)| contender(d)).collect();
        r
    }

    /// Why a winner that beats every seen entry still cannot be stated.
    fn loc_doubts(
        &self,
        w: &LocSite,
        wc: &Cand<'_>,
        tier: u8,
        (readings, all): (&Readings, &[[usize; 4]]),
    ) -> Gaps {
        let mut gaps = Gaps::new();
        if !self.derived.is_surviving(w.file_slot) {
            let pg = self.derived.path_gaps(w.site.path);
            gaps.extend(pg.iter().copied());
            if pg.is_empty() {
                gaps.push(ResolutionGap::ActivationUnknown);
            }
        }
        if self.activation_of(w.site.source) != Activation::Active {
            gaps.push(ResolutionGap::ActivationUnknown);
        }
        let scope = self.class_of(w.site.path).domain;
        // An unclassified scope tracks no incomplete files, so nothing can be ruled out.
        let beats_phantoms = scope.is_some_and(|d| {
            self.defs
                .incomplete_in(d)
                .iter()
                .filter(|f| self.derived.contends(f.slot))
                .filter(|f| self.class_of(f.path).tier >= tier)
                .all(|f| {
                    let ph = self.cand(f.source, f.path, None);
                    self.class_of(f.path).tier == tier
                        && all.iter().all(|ix| {
                            matches!(
                                compare(&self.sources, wc, &ph, readings.at(*ix)),
                                Ok((Ordering::Greater, _))
                            )
                        })
                })
        });
        if !beats_phantoms {
            gaps.push(ResolutionGap::ParseIncomplete);
        }
        gaps
    }
}
