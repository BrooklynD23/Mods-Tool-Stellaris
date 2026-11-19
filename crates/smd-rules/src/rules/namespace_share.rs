//! `pdx.namespace_share` — unrelated mods declaring one event namespace.
//!
//! No collision yet, so no winner: every participant is a `Contributor`. "Unrelated" means
//! at least one pair of the sharing mods has no resolved dependency either way; vanilla
//! and DLC are not mods here, since overriding their events means declaring their
//! namespace. Declarations whose file only *possibly* survives carry that path's gaps; an
//! incomplete file in the namespace's scope adds `ParseIncomplete` (more sharers may hide
//! there), and an unknown activation adds `ActivationUnknown`.

use smallvec::SmallVec;
use smd_core::{
    Activation, Capabilities, DomainId, EvidenceBag, EvidenceType, EvidenceValue, Finding,
    Participant, ResolutionGap, ResolvedDependency, Role, RuleId, SourceId, SourceRole, Subject,
    SymbolId, SymbolKey,
};
use smd_index::{DefSite, ScanIndex};

use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct NamespaceShare;

impl NamespaceShare {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(NamespaceShare::CASES);

impl Rule for NamespaceShare {
    fn id(&self) -> RuleId {
        RuleId("pdx.namespace_share")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::EVENTS | Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("sources", EvidenceType::Int)]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (ns, sites) in ctx.index.namespaces().multi_defined() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let live: Vec<DefSite> =
                sites.iter().filter(|d| sharer(ctx.index, d)).copied().collect();
            let mut mods: Vec<SourceId> = live.iter().map(|d| d.site.source).collect();
            mods.dedup();
            if mods.len() >= 2 && any_unrelated(ctx.index, &mods) {
                out.push(self.finding(ctx.index, *ns, &live, mods.len()));
            }
        }
        out
    }
}

/// A live declaration by a non-`Inactive` mod (not vanilla, not DLC).
fn sharer(index: &ScanIndex, d: &DefSite) -> bool {
    let Some(m) = index.sources().get(d.site.source) else { return false };
    m.activation != Activation::Inactive
        && !matches!(m.role, SourceRole::Vanilla | SourceRole::Dlc)
        && index.derived().contends(d.file_slot)
}

fn depends(index: &ScanIndex, a: SourceId, b: SourceId) -> bool {
    index.sources().get(a).is_some_and(|m| {
        m.deps
            .iter()
            .any(|d| matches!(d, ResolvedDependency::Resolved { source, .. } if *source == b))
    })
}

fn any_unrelated(index: &ScanIndex, mods: &[SourceId]) -> bool {
    mods.iter().enumerate().any(|(i, a)| {
        mods[i + 1..].iter().any(|b| !depends(index, *a, *b) && !depends(index, *b, *a))
    })
}

impl NamespaceShare {
    fn finding(&self, index: &ScanIndex, ns: SymbolId, live: &[DefSite], n: usize) -> Finding {
        let domain = index.class_of(live[0].site.path).domain;
        let mut gaps: SmallVec<[ResolutionGap; 1]> = live
            .iter()
            .flat_map(|d| index.derived().path_gaps(d.site.path).iter().copied())
            .collect();
        let unknown = |d: &DefSite| {
            index.sources().get(d.site.source).is_none_or(|m| m.activation != Activation::Active)
        };
        if live.iter().any(unknown) {
            gaps.push(ResolutionGap::ActivationUnknown);
        }
        if scope_incomplete(index, domain) {
            gaps.push(ResolutionGap::ParseIncomplete);
        }
        gaps.sort();
        gaps.dedup();
        let mut participants: Vec<Participant> =
            live.iter().map(|d| Participant { site: d.site, role: Role::Contributor }).collect();
        participants.sort_by_key(|p| p.site.sort_key());
        let mut facts = EvidenceBag::new();
        facts.push(("sources", EvidenceValue::Int(i64::try_from(n).unwrap_or(i64::MAX))));
        // ponytail: an unclassified path has no domain; the subject then shows domain 65535.
        let domain = domain.unwrap_or(DomainId(u16::MAX));
        Finding {
            rule: self.id(),
            subject: Subject::Symbol(SymbolKey { domain, symbol: ns }),
            participants,
            facts,
            gaps,
            ..Finding::unscored()
        }
    }
}

/// Whether a contending file of the namespace's scope (every scope when unclassified) was
/// not fully read.
fn scope_incomplete(index: &ScanIndex, domain: Option<DomainId>) -> bool {
    let all = domain.is_none();
    index
        .defs()
        .incomplete_all()
        .any(|(d, f)| (all || Some(d) == domain) && index.derived().contends(f.slot))
}
