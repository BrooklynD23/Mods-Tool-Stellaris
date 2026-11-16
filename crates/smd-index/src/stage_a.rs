//! Stage A — the file overlay (doc 07 § Stage A). One path in, one [`PathVerdict`] out.

use std::cmp::Ordering;

use smallvec::SmallVec;
use smd_core::{
    Activation, Agreement, Blob, Contender, Mechanism, Outcome, ParseCompleteness, PathId,
    Resolution, ResolutionGap, SamePath, ShadowCause, SourceTable, UnparsedReason,
};

use crate::overlay::Contribution;
use crate::overlay_rules::ReplacePathScope;
use crate::replace::{Doubt, Fate, FateCtx, ReplaceDecl, contender, fate};

/// Stage A's answer for one path, with the structure Stage B and the rules need.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PathVerdict {
    /// The resolution `resolve_path` returns.
    pub resolution: Resolution,
    /// Every participant: non-`Inactive` and not stated-dropped by `replace_path`,
    /// ascending by `SourceId::index()`. Includes same-path losers.
    pub live: SmallVec<[Contender; 2]>,
    /// Every stated shadowing on this path, ascending by source index.
    pub shadows: SmallVec<[(Contender, ShadowCause); 2]>,
}

/// Inputs for one path.
pub(crate) struct PathInput<'a> {
    pub sources: &'a SourceTable,
    pub path_id: PathId,
    pub path: &'a str,
    pub contribs: &'a [Contribution],
    pub same_path: SamePath,
    pub decls: &'a [ReplaceDecl],
    pub scope: ReplacePathScope,
}

type LiveVec<'a> = SmallVec<[(&'a Contribution, Doubt); 2]>;
type ShadowVec = SmallVec<[(Contender, ShadowCause); 2]>;
type Live<'a> = [(&'a Contribution, Doubt)];

/// Whether the bytes of this copy, or whether the game loads it at all, are unknown.
/// `Refused` (an unfollowed link, a cloud placeholder, a non-regular file, an escaped
/// handle) carries no blob, and the game may well follow what the walker did not (doc 05),
/// so it is a gap, not a fact. `Oversize` is left out on purpose: doc 07 lets it resolve.
fn bytes_unknown(c: &Contribution) -> bool {
    matches!(
        c.completeness,
        ParseCompleteness::Unparsed(
            UnparsedReason::Unreadable
                | UnparsedReason::ChangedDuringScan
                | UnparsedReason::Refused
        )
    )
}

fn activation(sources: &SourceTable, c: &Contribution) -> Activation {
    sources.get(c.source).map_or(Activation::Unknown, |m| m.activation)
}

/// The unique contribution that out-ranks all the others, if the ranks say there is one.
fn find_top(sources: &SourceTable, live: &Live<'_>) -> Option<usize> {
    let gt = |a: usize, b: usize| {
        sources.precedes(live[a].0.source, live[b].0.source) == Some(Ordering::Greater)
    };
    let mut best = 0;
    for i in 1..live.len() {
        if gt(i, best) {
            best = i;
        }
    }
    (0..live.len()).all(|j| j == best || gt(best, j)).then_some(best)
}

fn push_all(gaps: &mut Vec<ResolutionGap>, more: impl IntoIterator<Item = ResolutionGap>) {
    gaps.extend(more);
}

/// Which participant, if any, is the stated winner; otherwise why not. Every gap that
/// applies is recorded, including those an earlier blocker would already mask.
fn analyse(inp: &PathInput<'_>, live: &Live<'_>) -> (Option<usize>, Vec<ResolutionGap>) {
    let mut gaps = Vec::new();
    if live.is_empty() {
        return (None, gaps);
    }
    if live.iter().any(|(c, _)| bytes_unknown(c)) {
        gaps.push(ResolutionGap::ParseIncomplete);
    }
    if live.iter().any(|(c, _)| c.ambiguous) {
        gaps.push(ResolutionGap::IdentityCollision);
    }
    if live.len() >= 2 && inp.same_path != SamePath::Replace {
        gaps.push(ResolutionGap::SamePathUnknown);
    }
    let top = if live.len() == 1 { Some(0) } else { find_top(inp.sources, live) };
    match top {
        Some(i) => {
            if activation(inp.sources, live[i].0) != Activation::Active {
                gaps.push(ResolutionGap::ActivationUnknown);
            }
            push_all(&mut gaps, live[i].1.gaps());
        }
        None => {
            gaps.push(ResolutionGap::OrderUnknown);
            if live.iter().any(|(c, _)| activation(inp.sources, c) != Activation::Active) {
                gaps.push(ResolutionGap::ActivationUnknown);
            }
            let all = live.iter().fold(Doubt::default(), |d, (_, m)| d.union(*m));
            push_all(&mut gaps, all.gaps());
        }
    }
    gaps.sort();
    gaps.dedup();
    (if gaps.is_empty() { top } else { None }, gaps)
}

fn agreement(sources: &SourceTable, live: &Live<'_>, winner: Option<usize>) -> Agreement {
    let blobs: Option<Vec<Blob>> = live.iter().map(|(c, _)| c.blob).collect();
    let Some(blobs) = blobs.filter(|b| !b.is_empty()) else {
        return Agreement::Differs;
    };
    if blobs.iter().all(|b| *b == blobs[0]) {
        return Agreement::AllIdentical;
    }
    let Some(w) = winner else { return Agreement::Differs };
    let losers_match = live.iter().enumerate().all(|(i, (c, _))| {
        i == w
            || blobs[i] == blobs[w]
            || sources.get(c.source).is_some_and(|m| m.role == smd_core::SourceRole::Vanilla)
    });
    if losers_match { Agreement::LosersMatchWinner } else { Agreement::Differs }
}

/// Decide which contributions are participants and which `replace_path` already dropped.
fn partition<'a>(inp: &PathInput<'a>) -> (LiveVec<'a>, ShadowVec) {
    let cx = FateCtx { sources: inp.sources, decls: inp.decls, scope: inp.scope, path: inp.path };
    let (mut live, mut shadows) = (SmallVec::new(), SmallVec::new());
    for c in inp.contribs {
        if activation(inp.sources, c) == Activation::Inactive {
            continue;
        }
        match fate(&cx, c) {
            Fate::Keep => live.push((c, Doubt::default())),
            Fate::Maybe(d) => live.push((c, d)),
            Fate::Drop(by) => {
                shadows.push((contender(c, inp.path_id), ShadowCause::ReplacePath(by)))
            }
        }
    }
    (live, shadows)
}

/// Resolve one path.
pub(crate) fn resolve(inp: &PathInput<'_>) -> PathVerdict {
    let (live, mut shadows) = partition(inp);
    let (winner, gaps) = analyse(inp, &live);
    let contenders: SmallVec<[Contender; 2]> =
        live.iter().map(|(c, _)| contender(c, inp.path_id)).collect();
    let replace_drops = shadows.len();
    let mut resolution = Resolution {
        outcome: Outcome::Unknown,
        winner: None,
        discarded: SmallVec::new(),
        agreement: agreement(inp.sources, &live, winner),
        mechanism: Mechanism::Undetermined,
        gaps: gaps.into_iter().collect(),
    };
    if let Some(w) = winner {
        resolution.outcome = Outcome::SingleWinner;
        resolution.winner = Some(contenders[w]);
        for c in contenders.iter().enumerate().filter(|(i, _)| *i != w).map(|(_, c)| c) {
            shadows.push((*c, ShadowCause::SamePath(contenders[w].source)));
        }
        resolution.mechanism = match (shadows.len() > replace_drops, replace_drops > 0) {
            (true, _) => Mechanism::SamePath,
            (false, true) => Mechanism::ReplacePath,
            // A lone stated contributor: nothing was shadowed, and the file simply stands.
            _ => Mechanism::SamePath,
        };
    } else if live.is_empty() && !shadows.is_empty() {
        resolution.outcome = Outcome::Deleted;
        resolution.mechanism = Mechanism::ReplacePath;
    }
    resolution.discarded = shadows.iter().map(|(c, _)| *c).collect();
    shadows.sort_by_key(|(c, _)| c.source.index());
    PathVerdict { resolution, live: contenders, shadows }
}
