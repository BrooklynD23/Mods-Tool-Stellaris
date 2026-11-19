//! `core.symbol_collision` — two or more live definitions of one `(domain, symbol)`.
//!
//! "Live" is `Derived::contends`: stated *or possible* survivors of Stage A, so under a
//! directory scan the collision is still reported, conditionally, without a winner. The
//! 1 → 0 case (the only definer's file replaced away) is `core.symbol_deleted`'s.
//!
//! In a `merge_by_key_additive_body` directory (H4) the resolver compares residuals:
//! equal is `Coexist`, unequal is `PartiallyMerged` with `residual_lost = true`, and a
//! participant without an `AdditiveSplit` is `Unknown` + `child_digests_unavailable`.
//! Doc 08 names no separate v1 rule for additive overlap (`pdx.on_action_overlap` is
//! post-v1), so this rule carries it.

use smd_core::{
    Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, Outcome, RuleId, Subject,
    SymbolKey,
};
use smd_index::{ScanIndex, SymbolVerdict};

use super::symbols::{domain, participants};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct SymbolCollision;

impl SymbolCollision {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(SymbolCollision::CASES);

impl Rule for SymbolCollision {
    fn id(&self) -> RuleId {
        RuleId("core.symbol_collision")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DEFINITIONS | Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[
            ("agreement", EvidenceType::Tag("Agreement")),
            ("domain", EvidenceType::Sym),
            ("mechanism", EvidenceType::Tag("Mechanism")),
            ("merge", EvidenceType::Tag("MergeMode")),
            ("residual_lost", EvidenceType::Bool),
        ]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (key, sites) in ctx.index.defs().multi_defined() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            if sites.iter().filter(|d| ctx.index.derived().contends(d.file_slot)).count() < 2 {
                continue;
            }
            let v = ctx.index.resolve_symbol(key);
            if v.live.len() >= 2 {
                out.push(self.finding(ctx.index, key, &v));
            }
        }
        out
    }
}

impl SymbolCollision {
    fn finding(&self, index: &ScanIndex, key: SymbolKey, v: &SymbolVerdict) -> Finding {
        let r = &v.resolution;
        let mut facts = EvidenceBag::new();
        facts.push(("agreement", EvidenceValue::Tag(r.agreement.as_tag())));
        domain(index, key, &mut facts);
        facts.push(("mechanism", EvidenceValue::Tag(r.mechanism.as_tag())));
        let merge = index.class_of(v.live[0].site.path).merge;
        facts.push(("merge", EvidenceValue::Tag(merge.as_tag())));
        if r.outcome == Outcome::PartiallyMerged {
            // H4 (doc 07 R1): the losers keep their additive children, so they are
            // Contributors, but their non-additive children do not reach the game.
            facts.push(("residual_lost", EvidenceValue::Bool(true)));
        }
        Finding {
            rule: self.id(),
            subject: Subject::Symbol(key),
            participants: participants(index, v, &v.live),
            facts,
            gaps: r.gaps.clone(),
            ..Finding::unscored()
        }
    }
}
