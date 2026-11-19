//! Stage D — the precedence tuple, under every reading of the unknown parameters
//! (doc 07 § Stage D, § Activation, unknown parameters, and incompleteness, rule 2).
//!
//! Order of comparison: in-file ordinal (same file), vanilla-lowest (when vanilla is not
//! on the filename axis), the filename key in the directory's direction, then `rank`.
//! `tier` is 0 for every script path and is not compared yet.

use std::cmp::Ordering;

use smallvec::SmallVec;
use smd_core::{FileOrder, PathId, ResolutionGap, SourceId, SourceTable};
use smd_profiles::{FilenameKey, KeyReadings};

/// One assignment of every Stage D parameter.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Reading {
    pub key: FilenameKey,
    pub vanilla_in: bool,
    pub in_file_last: bool,
    pub order: FileOrder,
}

/// The value sets, one per parameter, in a fixed order: key, vanilla, in-file, order.
pub(crate) struct Readings {
    keys: SmallVec<[FilenameKey; 3]>,
    vanilla: SmallVec<[bool; 2]>,
    in_file: SmallVec<[bool; 2]>,
    order: SmallVec<[FileOrder; 2]>,
}

/// The gap a parameter's disagreement is reported as, by parameter index.
pub(crate) const PARAM_GAPS: [ResolutionGap; 4] = [
    ResolutionGap::FilenameKeyForm,
    ResolutionGap::VanillaFilenameAxis,
    ResolutionGap::InFileOrder,
    // ponytail: no gap names an unknown filename *direction*; `OrderUnknown` is the
    // nearest. Add a variant when a profile actually ships `order = "unknown"`.
    ResolutionGap::OrderUnknown,
];

impl Readings {
    /// Known parameters take their one value; unknown ones every value.
    pub(crate) fn new(r: KeyReadings, order: Option<FileOrder>) -> Readings {
        use FilenameKey::{Basename, BasenameNoExt, DirRelative};
        let both = |v: Option<bool>| {
            v.map_or(SmallVec::from_slice(&[false, true]), |b| SmallVec::from_slice(&[b]))
        };
        Readings {
            keys: r
                .filename_key
                .map_or(SmallVec::from_slice(&[BasenameNoExt, Basename, DirRelative]), |k| {
                    SmallVec::from_slice(&[k])
                }),
            vanilla: both(r.vanilla_in_axis),
            in_file: both(r.in_file_last),
            order: order.map_or(SmallVec::from_slice(&[FileOrder::Lios, FileOrder::Fios]), |o| {
                SmallVec::from_slice(&[o])
            }),
        }
    }

    /// Index bounds per parameter.
    pub(crate) fn dims(&self) -> [usize; 4] {
        [self.keys.len(), self.vanilla.len(), self.in_file.len(), self.order.len()]
    }

    /// The reading at `ix`.
    pub(crate) fn at(&self, ix: [usize; 4]) -> Reading {
        Reading {
            key: self.keys[ix[0]],
            vanilla_in: self.vanilla[ix[1]],
            in_file_last: self.in_file[ix[2]],
            order: self.order[ix[3]],
        }
    }

    /// Every index tuple, in a fixed order.
    pub(crate) fn all(&self) -> Vec<[usize; 4]> {
        let d = self.dims();
        let mut out = Vec::with_capacity(d.iter().product());
        for a in 0..d[0] {
            for b in 0..d[1] {
                for c in 0..d[2] {
                    for e in 0..d[3] {
                        out.push([a, b, c, e]);
                    }
                }
            }
        }
        out
    }
}

/// One contender as Stage D sees it. `ordinal: None` is an incomplete file standing in
/// for definitions that were not seen.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Cand<'a> {
    pub source: SourceId,
    pub path: PathId,
    /// Normalised directory part of the path.
    pub dir: &'a str,
    /// As-shipped file name.
    pub raw: &'a str,
    pub ordinal: Option<u32>,
    pub vanilla: bool,
}

/// Which component decided a comparison.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum Decider {
    Filename,
    Rank,
}

fn stem(raw: &str) -> &str {
    raw.rsplit_once('.').map_or(raw, |(s, _)| s)
}

fn key_cmp(a: &Cand<'_>, b: &Cand<'_>, k: FilenameKey) -> Ordering {
    match k {
        FilenameKey::BasenameNoExt => stem(a.raw).cmp(stem(b.raw)),
        FilenameKey::Basename => a.raw.cmp(b.raw),
        // ponytail: the directory part is the normalised (lowercased) path, so case in
        // directory names does not order; only the file name keeps its shipped case.
        FilenameKey::DirRelative => (a.dir, a.raw).cmp(&(b.dir, b.raw)),
    }
}

fn rank(t: &SourceTable, a: &Cand<'_>, b: &Cand<'_>) -> Result<(Ordering, Decider), ResolutionGap> {
    t.precedes(a.source, b.source).map(|o| (o, Decider::Rank)).ok_or(ResolutionGap::OrderUnknown)
}

/// `Greater` = `a` wins.
pub(crate) fn compare(
    t: &SourceTable,
    a: &Cand<'_>,
    b: &Cand<'_>,
    r: Reading,
) -> Result<(Ordering, Decider), ResolutionGap> {
    if a.path == b.path && a.source == b.source {
        let (Some(x), Some(y)) = (a.ordinal, b.ordinal) else {
            return Err(ResolutionGap::ParseIncomplete);
        };
        let o = if r.in_file_last { x.cmp(&y) } else { y.cmp(&x) };
        return Ok((o, Decider::Filename));
    }
    if a.path == b.path {
        return rank(t, a, b);
    }
    if !r.vanilla_in && a.vanilla != b.vanilla {
        return Ok((if a.vanilla { Ordering::Less } else { Ordering::Greater }, Decider::Rank));
    }
    match key_cmp(a, b, r.key) {
        Ordering::Equal if a.source == b.source => Err(ResolutionGap::FileKeyTie),
        Ordering::Equal => rank(t, a, b),
        o => Ok((if r.order == FileOrder::Lios { o } else { o.reverse() }, Decider::Filename)),
    }
}

/// The candidate that beats every other under `r`, or the gaps that kept any from doing so.
/// ponytail: O(n²) comparisons per reading; groups are a handful of contenders.
pub(crate) fn top(
    t: &SourceTable,
    cands: &[Cand<'_>],
    r: Reading,
) -> Result<usize, SmallVec<[ResolutionGap; 2]>> {
    let mut gaps: SmallVec<[ResolutionGap; 2]> = SmallVec::new();
    for i in 0..cands.len() {
        let mut beats_all = true;
        for j in (0..cands.len()).filter(|&j| j != i) {
            match compare(t, &cands[i], &cands[j], r) {
                Ok((Ordering::Greater, _)) => {}
                Ok(_) => beats_all = false,
                Err(g) => {
                    beats_all = false;
                    gaps.push(g);
                }
            }
        }
        if beats_all {
            return Ok(i);
        }
    }
    gaps.sort();
    gaps.dedup();
    if gaps.is_empty() {
        gaps.push(ResolutionGap::OrderUnknown);
    }
    Err(gaps)
}
