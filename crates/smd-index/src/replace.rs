//! `replace_path` declarations and the Stage A fate of a contribution under them.

use smd_core::{Activation, Contender, ResolutionGap, SourceId, SourceRole, SourceTable};
use std::cmp::Ordering;

use crate::overlay::Contribution;
use crate::overlay_rules::ReplacePathScope;

/// One source's declared `replace_path` prefix, normalised.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReplaceDecl {
    /// Who declared it.
    pub declarer: SourceId,
    /// Lowercase, forward slashes, no leading or trailing slash.
    pub prefix: Box<str>,
}

/// Normalise a declared prefix the way paths are normalised.
#[must_use]
pub fn normalise_prefix(raw: &str) -> String {
    raw.replace('\\', "/").trim_matches('/').to_lowercase()
}

/// Whether `path` is `prefix` or lies under it.
#[must_use]
pub fn covers(prefix: &str, path: &str) -> bool {
    !prefix.is_empty()
        && path.strip_prefix(prefix).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// Which gaps a possibly-dropped contribution carries.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct Doubt(u8);

impl Doubt {
    const SCOPE: u8 = 1;
    const ORDER: u8 = 2;
    const ACTIVATION: u8 = 4;

    /// Whether any doubt is recorded.
    #[must_use]
    pub fn any(self) -> bool {
        self.0 != 0
    }

    fn with(self, bits: u8) -> Doubt {
        Doubt(self.0 | bits)
    }

    /// Union of two doubts.
    #[must_use]
    pub fn union(self, other: Doubt) -> Doubt {
        Doubt(self.0 | other.0)
    }

    /// The gaps, in `ResolutionGap` order.
    #[must_use]
    pub fn gaps(self) -> Vec<ResolutionGap> {
        let mut v = Vec::new();
        if self.0 & Self::ORDER != 0 {
            v.push(ResolutionGap::OrderUnknown);
        }
        if self.0 & Self::ACTIVATION != 0 {
            v.push(ResolutionGap::ActivationUnknown);
        }
        if self.0 & Self::SCOPE != 0 {
            v.push(ResolutionGap::ReplacePathScope);
        }
        v
    }
}

/// What the declarations say about one contribution.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Fate {
    /// Nothing drops it.
    Keep,
    /// An `Active` declarer that outranks it stated-drops it.
    Drop(SourceId),
    /// Dropped under some reading of an unknown input.
    Maybe(Doubt),
}

enum Verdict {
    Keep,
    Drop,
    Unsure(u8),
}

/// What Stage A needs to decide a fate.
pub struct FateCtx<'a> {
    /// Rank and activation.
    pub sources: &'a SourceTable,
    /// Every declaration, any order.
    pub decls: &'a [ReplaceDecl],
    /// The profile's scope reading.
    pub scope: ReplacePathScope,
    /// The path under test.
    pub path: &'a str,
}

fn is_vanilla(sources: &SourceTable, id: SourceId) -> bool {
    id == SourceId::VANILLA || sources.get(id).is_some_and(|m| m.role == SourceRole::Vanilla)
}

fn rank_verdict(sources: &SourceTable, declarer: SourceId, target: SourceId) -> Verdict {
    match sources.precedes(declarer, target) {
        Some(Ordering::Greater) => Verdict::Drop,
        Some(_) => Verdict::Keep,
        None => Verdict::Unsure(Doubt::ORDER),
    }
}

fn verdict(cx: &FateCtx<'_>, declarer: SourceId, target: SourceId) -> Verdict {
    if is_vanilla(cx.sources, target) {
        return Verdict::Drop;
    }
    // Whether `replace_path` treats a DLC as base game is not settled (docs 07 and 10), so a
    // DLC target is an enumerated unknown under every scope reading.
    if cx.sources.get(target).is_some_and(|m| m.role == SourceRole::Dlc) {
        return match rank_verdict(cx.sources, declarer, target) {
            Verdict::Unsure(bits) => Verdict::Unsure(bits | Doubt::SCOPE),
            _ => Verdict::Unsure(Doubt::SCOPE),
        };
    }
    match cx.scope {
        ReplacePathScope::VanillaOnly => Verdict::Keep,
        ReplacePathScope::VanillaAndEarlier => rank_verdict(cx.sources, declarer, target),
        ReplacePathScope::Unknown => match rank_verdict(cx.sources, declarer, target) {
            Verdict::Keep => Verdict::Keep,
            Verdict::Drop => Verdict::Unsure(Doubt::SCOPE),
            Verdict::Unsure(bits) => Verdict::Unsure(bits | Doubt::SCOPE),
        },
    }
}

/// The fate of one contribution under every declaration covering its path.
#[must_use]
pub fn fate(cx: &FateCtx<'_>, c: &Contribution) -> Fate {
    let mut dropper: Option<SourceId> = None;
    let mut doubt = Doubt::default();
    for d in cx.decls.iter().filter(|d| covers(&d.prefix, cx.path)) {
        let declarer = d.declarer;
        let Some(meta) = cx.sources.get(declarer) else { continue };
        if declarer == c.source || meta.activation == Activation::Inactive {
            continue;
        }
        let unknown = if meta.activation == Activation::Unknown { Doubt::ACTIVATION } else { 0 };
        match verdict(cx, declarer, c.source) {
            Verdict::Keep => {}
            Verdict::Drop if unknown == 0 => {
                dropper.get_or_insert(declarer);
            }
            Verdict::Drop => doubt = doubt.with(unknown),
            Verdict::Unsure(bits) => doubt = doubt.with(bits | unknown),
        }
    }
    match dropper {
        Some(d) => Fate::Drop(d),
        None if doubt.any() => Fate::Maybe(doubt),
        None => Fate::Keep,
    }
}

/// A [`Contender`] for a whole file.
#[must_use]
pub fn contender(c: &Contribution, path: smd_core::PathId) -> Contender {
    Contender { source: c.source, path, slot: None }
}
