//! `dep.missing` — a mod declares a dependency (by display name) that is not found among
//! the discovered mods, or is found but not switched on.
//!
//! Reads [`SourceMeta::deps`](smd_core::SourceMeta::deps), resolved by name in ingest. An
//! ambiguous name (several matches) is not reported: it is not missing. "Not found" is
//! never a fact on incomplete discovery: the engine lowers completeness then. An
//! `Unknown` activation on either side is reported as `activation_unknown` with a gap,
//! never as "not enabled".
//!
//! Untested: no cases yet (testing deferred by the owner).

use smallvec::SmallVec;
use smd_core::{
    Activation, ActivationCertainty, Capabilities, EvidenceBag, EvidenceType, EvidenceValue,
    Finding, ResolutionGap, ResolvedDependency, Role, RuleId, SourceId, Subject,
};
use smd_index::ScanIndex;

use super::playset::{STATED, maybe_loaded, mod_site, text};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

/// See the module docs.
pub struct DepMissing;

impl Rule for DepMissing {
    fn id(&self) -> RuleId {
        RuleId("dep.missing")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DESCRIPTORS
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("dependency", EvidenceType::Sym), ("state", EvidenceType::Tag("DepState"))]
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
            out.extend(meta.deps.iter().filter_map(|d| self.check(ctx.index, me, d)));
        }
        out
    }
}

impl DepMissing {
    fn check(&self, index: &ScanIndex, me: SourceId, d: &ResolvedDependency) -> Option<Finding> {
        let (declared, dep, state) = match d {
            ResolvedDependency::Missing { declared, candidates } if candidates.is_empty() => {
                (declared, None, "not_found")
            }
            ResolvedDependency::Resolved { declared, source } => {
                let state = match index.sources().get(*source)?.activation {
                    Activation::Active => return None,
                    Activation::Inactive => "not_enabled",
                    _ => "activation_unknown",
                };
                (declared, Some(*source), state)
            }
            _ => return None,
        };
        let mut participants = vec![mod_site(me, Role::Contributor)];
        participants.extend(dep.map(|s| mod_site(s, Role::Missing)));
        let unknown = |s: SourceId| {
            index.sources().get(s).is_none_or(|m| m.activation == Activation::Unknown)
        };
        let mut gaps = SmallVec::new();
        let mut certainty = STATED;
        if unknown(me) || dep.is_some_and(unknown) {
            gaps.push(ResolutionGap::ActivationUnknown);
            certainty.activation = ActivationCertainty::Conditional;
        }
        let mut facts = EvidenceBag::new();
        facts.push(("dependency", text(index, declared)));
        facts.push(("state", EvidenceValue::Tag(state)));
        let subject = dep.map_or(Subject::Mod(me), |s| Subject::ModPair(me, s));
        Some(Finding {
            rule: self.id(),
            subject,
            participants,
            facts,
            certainty,
            gaps,
            ..Finding::unscored()
        })
    }
}
