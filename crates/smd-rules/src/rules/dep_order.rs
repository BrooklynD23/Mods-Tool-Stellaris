//! `dep.order` — a resolved dependency loads after the mod that needs it.
//!
//! Precedence is compared only through `SourceTable::precedes`. When either rank is empty
//! the order is not known, so the finding says `unknown` with an `OrderUnknown` gap
//! rather than claiming either order. A dependency known to be off is `dep.missing`'s.
//!
//! Untested: no cases yet (testing deferred by the owner).

use std::cmp::Ordering;

use smallvec::SmallVec;
use smd_core::{
    Capabilities, Confidence, EvidenceBag, EvidenceType, EvidenceValue, Finding, ResolutionGap,
    ResolvedDependency, Role, RuleId, SourceId, Subject,
};
use smd_index::ScanIndex;

use super::playset::{STATED, maybe_loaded, mod_site, text};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

/// See the module docs.
pub struct DepOrder;

impl Rule for DepOrder {
    fn id(&self) -> RuleId {
        RuleId("dep.order")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DESCRIPTORS
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("dependency", EvidenceType::Sym), ("order", EvidenceType::Tag("DepOrder"))]
    }

    fn cases(&self) -> &[RuleCase] {
        &[]
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (me, meta) in ctx.index.sources().iter() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            if !maybe_loaded(ctx.index, me) {
                continue;
            }
            for d in &meta.deps {
                if let ResolvedDependency::Resolved { declared, source } = d {
                    out.extend(self.check(ctx.index, me, *source, declared));
                }
            }
        }
        out
    }
}

impl DepOrder {
    fn check(&self, index: &ScanIndex, me: SourceId, dep: SourceId, name: &str) -> Option<Finding> {
        if !maybe_loaded(index, dep) {
            return None;
        }
        let mut gaps = SmallVec::new();
        let mut certainty = STATED;
        let order = match index.sources().precedes(dep, me) {
            Some(Ordering::Greater) => "after",
            Some(_) => return None,
            None => {
                gaps.push(ResolutionGap::OrderUnknown);
                certainty.order = Confidence::Fallback;
                "unknown"
            }
        };
        let mut facts = EvidenceBag::new();
        facts.push(("dependency", text(index, name)));
        facts.push(("order", EvidenceValue::Tag(order)));
        Some(Finding {
            rule: self.id(),
            subject: Subject::ModPair(me, dep),
            participants: vec![mod_site(me, Role::Contributor), mod_site(dep, Role::Contributor)],
            facts,
            certainty,
            gaps,
            ..Finding::unscored()
        })
    }
}
