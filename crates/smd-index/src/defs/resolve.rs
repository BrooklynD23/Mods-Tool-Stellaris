//! Stage C (key resolution) and the `Deleted` test (doc 07 § Stage C, § Deleted symbols).
//!
//! Contenders are the definitions whose file contends after Stage B. A possibly-surviving
//! contender can block a winner but is never the stated winner; a winner is stated only if
//! it is surviving, `Active`, beats every contender under every reading of the unknown
//! Stage D parameters, and beats every incomplete file in its domain (an unseen definition
//! there could win). `Deleted` admits no such exception: any incomplete file or
//! content-incomplete source in scope makes it `Unknown` with `ParseIncomplete`.

use std::cmp::Ordering;

use smallvec::SmallVec;
use smd_core::{
    Activation, Agreement, Contender, DomainId, Mechanism, MergeMode, Outcome, PathId, Resolution,
    ResolutionGap, SourceId, SourceRole, SymbolKey,
};

use super::DefSite;
use super::stage_d::{Cand, PARAM_GAPS, Readings, compare, decided_by, top};
use crate::index::ScanIndex;

/// Stage C's answer for one key, with its participants.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SymbolVerdict {
    /// The resolution.
    pub resolution: Resolution,
    /// Definitions whose file contends (stated or possible survivor), not `Inactive`.
    pub live: SmallVec<[DefSite; 2]>,
    /// Definitions of non-`Inactive` sources whose file Stage A dropped.
    pub dead: SmallVec<[DefSite; 2]>,
}

type Gaps = SmallVec<[ResolutionGap; 1]>;

fn contender(d: &DefSite) -> Contender {
    Contender { source: d.site.source, path: d.site.path, slot: Some(d.file_slot) }
}

fn unknown(gaps: Gaps) -> Resolution {
    Resolution {
        outcome: Outcome::Unknown,
        winner: None,
        discarded: SmallVec::new(),
        agreement: Agreement::Differs,
        mechanism: Mechanism::Undetermined,
        gaps,
    }
}

fn stated(outcome: Outcome, mechanism: Mechanism, agreement: Agreement) -> Resolution {
    Resolution { outcome, mechanism, agreement, ..unknown(Gaps::new()) }
}

impl ScanIndex {
    fn activation_of(&self, s: SourceId) -> Activation {
        self.sources.get(s).map_or(Activation::Unknown, |m| m.activation)
    }

    fn is_vanilla(&self, s: SourceId) -> bool {
        self.sources.get(s).is_some_and(|m| m.role == SourceRole::Vanilla)
    }

    fn cand(&self, source: SourceId, path: PathId, ordinal: Option<u32>) -> Cand<'_> {
        let text = self.path_text(path);
        let raw = self
            .overlay
            .contributions(path)
            .iter()
            .find(|c| c.source == source)
            .map_or("", |c| self.symbols.get(c.raw_name.0));
        let dir = text.rsplit_once('/').map_or("", |(d, _)| d);
        let tier = self.class_of(path).tier;
        Cand { source, path, dir, raw, ordinal, vanilla: self.is_vanilla(source), tier }
    }

    /// Stages C and D for one key.
    #[must_use]
    pub fn resolve_symbol(&self, key: SymbolKey) -> SymbolVerdict {
        self.resolve_sites(Some(key.domain), self.defs.sites(key))
    }

    /// Stages C and D for one key group of any table. `domain` is the merge scope whose
    /// incomplete files can hide a definition; `None` (an unclassified path) counts every
    /// incomplete file in the scan.
    pub(crate) fn resolve_sites(
        &self,
        domain: Option<DomainId>,
        sites: &[DefSite],
    ) -> SymbolVerdict {
        let (mut live, mut dead) = (SmallVec::new(), SmallVec::new());
        for d in sites {
            if self.activation_of(d.site.source) == Activation::Inactive {
                continue;
            }
            if self.derived.contends(d.file_slot) { live.push(*d) } else { dead.push(*d) }
        }
        let resolution =
            if live.is_empty() { self.deleted(domain, &dead) } else { self.contest(domain, &live) };
        SymbolVerdict { resolution, live, dead }
    }

    /// Keys with at least one non-`Inactive` definition and none whose file contends:
    /// the candidates `resolve_symbol` answers `Deleted` or, when it cannot confirm that,
    /// `Unknown` with the blocking gaps. Ascending by key.
    pub fn no_live_site(&self) -> impl Iterator<Item = SymbolKey> + '_ {
        self.defs.iter().filter_map(|(k, sites)| {
            let mut active =
                sites.iter().filter(|d| self.activation_of(d.site.source) != Activation::Inactive);
            let first = active.next()?;
            let any_live =
                std::iter::once(first).chain(active).any(|d| self.derived.contends(d.file_slot));
            (!any_live).then_some(k)
        })
    }

    fn scope_incomplete(&self, domain: Option<DomainId>) -> bool {
        self.scope_files(domain).any(|f| self.derived.contends(f.slot))
    }

    /// The incomplete files of a merge scope; every domain's when the scope is unknown.
    fn scope_files(
        &self,
        domain: Option<DomainId>,
    ) -> impl Iterator<Item = &super::IncompleteFile> + '_ {
        let all = domain.is_none();
        self.defs.incomplete_all().filter(move |(d, _)| all || Some(*d) == domain).map(|(_, f)| f)
    }

    fn deleted(&self, key: Option<DomainId>, dead: &[DefSite]) -> Resolution {
        let Some(first) = dead.first() else { return unknown(Gaps::new()) };
        let mut gaps: Gaps =
            dead.iter().flat_map(|d| self.derived.path_gaps(d.site.path).iter().copied()).collect();
        let content_incomplete =
            self.content_incomplete.iter().any(|s| self.activation_of(*s) != Activation::Inactive);
        if content_incomplete || self.scope_incomplete(key) {
            gaps.push(ResolutionGap::ParseIncomplete);
        }
        gaps.sort();
        gaps.dedup();
        if !gaps.is_empty() {
            return unknown(gaps);
        }
        let mechanism = self.path_verdict(first.site.path).resolution.mechanism;
        let mut r = stated(Outcome::Deleted, mechanism, Agreement::Differs);
        r.discarded = dead.iter().map(contender).collect();
        r
    }

    fn contest(&self, key: Option<DomainId>, live: &[DefSite]) -> Resolution {
        let class = self.class_of(live[0].site.path);
        let bodies: SmallVec<[[u8; 16]; 2]> = live.iter().map(|d| d.body_hash).collect();
        match class.merge {
            MergeMode::MergeByKey => self.by_precedence(key, live, class.order, &bodies),
            MergeMode::MergeByKeyAdditiveBody => self.additive_body(key, live, class.order),
            MergeMode::Additive => {
                stated(Outcome::Coexist, Mechanism::DirectoryMerge, agreement(&bodies, None, &[]))
            }
            MergeMode::Single => stated(
                Outcome::MergedInternally,
                Mechanism::DirectoryMerge,
                agreement(&bodies, None, &[]),
            ),
            _ => unknown(SmallVec::from_slice(&[ResolutionGap::MergeModeUnknown])),
        }
    }

    /// Invariant A1: no residual on any participant is `ChildDigestsUnavailable`.
    fn additive_body(
        &self,
        key: Option<DomainId>,
        live: &[DefSite],
        order: Option<smd_core::FileOrder>,
    ) -> Resolution {
        let Some(res) =
            live.iter().map(|d| d.residual_hash).collect::<Option<SmallVec<[[u8; 16]; 2]>>>()
        else {
            return unknown(SmallVec::from_slice(&[ResolutionGap::ChildDigestsUnavailable]));
        };
        if res.iter().all(|h| *h == res[0]) {
            return stated(Outcome::Coexist, Mechanism::DirectoryMerge, Agreement::AllIdentical);
        }
        let mut r = self.by_precedence(key, live, order, &res);
        if r.outcome == Outcome::SingleWinner {
            // Every participant's additive children still apply: nobody is discarded.
            r.outcome = Outcome::PartiallyMerged;
            r.discarded.clear();
        }
        r
    }

    fn by_precedence(
        &self,
        key: Option<DomainId>,
        live: &[DefSite],
        order: Option<smd_core::FileOrder>,
        hashes: &[[u8; 16]],
    ) -> Resolution {
        let readings = Readings::new(self.readings, order);
        let cands: Vec<Cand<'_>> =
            live.iter().map(|d| self.cand(d.site.source, d.site.path, Some(d.ordinal))).collect();
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
            gaps.extend(self.winner_doubts(key, &live[i], &cands[i], (&readings, &all)));
        }
        gaps.sort();
        gaps.dedup();
        match winner {
            Some(i) if gaps.is_empty() => {
                let mechanism = decided_by(&self.sources, &cands, i, (&readings, &all));
                let vanilla: Vec<bool> =
                    live.iter().map(|d| self.is_vanilla(d.site.source)).collect();
                let mut r =
                    stated(Outcome::SingleWinner, mechanism, agreement(hashes, Some(i), &vanilla));
                r.winner = Some(contender(&live[i]));
                r.discarded = live
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, d)| contender(d))
                    .collect();
                r
            }
            _ => unknown(gaps),
        }
    }

    /// Why a winner that beats every seen contender still cannot be stated.
    fn winner_doubts(
        &self,
        key: Option<DomainId>,
        w: &DefSite,
        wc: &Cand<'_>,
        (readings, all): (&Readings, &[[usize; 4]]),
    ) -> Gaps {
        let mut gaps = Gaps::new();
        if !self.derived.is_surviving(w.file_slot) {
            // Possibly surviving: it can block, never win. The path's Stage A gaps say why.
            let pg = self.derived.path_gaps(w.site.path);
            gaps.extend(pg.iter().copied());
            if pg.is_empty() {
                gaps.push(ResolutionGap::ActivationUnknown);
            }
        }
        if self.activation_of(w.site.source) != Activation::Active {
            gaps.push(ResolutionGap::ActivationUnknown);
        }
        let beats_phantoms =
            self.scope_files(key).filter(|f| self.derived.contends(f.slot)).all(|f| {
                let ph = self.cand(f.source, f.path, None);
                all.iter().all(|ix| {
                    matches!(
                        compare(&self.sources, wc, &ph, readings.at(*ix)),
                        Ok((Ordering::Greater, _))
                    )
                })
            });
        if !beats_phantoms {
            gaps.push(ResolutionGap::ParseIncomplete);
        }
        gaps
    }
}

type Top = Result<usize, SmallVec<[ResolutionGap; 2]>>;

/// One gap per enumerated parameter whose value alone changes the result.
fn flips(readings: &Readings, all: &[[usize; 4]], results: &[Top]) -> Gaps {
    let dims = readings.dims();
    let winner = |ix: [usize; 4]| {
        all.iter().position(|x| *x == ix).and_then(|p| results[p].as_ref().ok().copied())
    };
    let mut gaps = Gaps::new();
    for (p, gap) in PARAM_GAPS.iter().enumerate() {
        let flipped = all.iter().any(|ix| {
            (0..dims[p]).any(|v| {
                let mut other = *ix;
                other[p] = v;
                winner(other) != winner(*ix)
            })
        });
        if flipped {
            gaps.push(*gap);
        }
    }
    gaps
}

/// Doc 07 § Agreement; vanilla's body is excluded from the losers' comparison.
fn agreement(hashes: &[[u8; 16]], winner: Option<usize>, vanilla: &[bool]) -> Agreement {
    if hashes.iter().all(|h| *h == hashes[0]) {
        return Agreement::AllIdentical;
    }
    let Some(w) = winner else { return Agreement::Differs };
    let losers_match = hashes
        .iter()
        .enumerate()
        .all(|(i, h)| i == w || *h == hashes[w] || vanilla.get(i).copied().unwrap_or(false));
    if losers_match { Agreement::LosersMatchWinner } else { Agreement::Differs }
}
