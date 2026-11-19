//! `core.symbol_deleted` — a `(domain, symbol)` defined on disk with no live definition.
//!
//! The one definitive negative in the catalogue, so it is never made on incomplete
//! evidence (doc 07 § Deleted symbols): with a Stage A gap on any of the symbol's paths,
//! an incomplete file in its domain, or a content-incomplete source, the outcome is
//! `Unknown` with the blocking gaps and the finding means *could not verify*, never
//! *deleted*. Silence would hide a possibly real deletion; certainty would invent one.

use smd_core::{
    Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, RuleId, Subject, SymbolKey,
};
use smd_index::{ScanIndex, SymbolVerdict};

use super::symbols::{domain, participants};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct SymbolDeleted;

impl SymbolDeleted {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(SymbolDeleted::CASES);

impl Rule for SymbolDeleted {
    fn id(&self) -> RuleId {
        RuleId("core.symbol_deleted")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DEFINITIONS | Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[
            ("domain", EvidenceType::Sym),
            ("mechanism", EvidenceType::Tag("Mechanism")),
            ("outcome", EvidenceType::Tag("Outcome")),
        ]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for key in ctx.index.no_live_site() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let v = ctx.index.resolve_symbol(key);
            if !v.dead.is_empty() {
                out.push(self.finding(ctx.index, key, &v));
            }
        }
        out
    }
}

impl SymbolDeleted {
    fn finding(&self, index: &ScanIndex, key: SymbolKey, v: &SymbolVerdict) -> Finding {
        let r = &v.resolution;
        let mut facts = EvidenceBag::new();
        domain(index, key, &mut facts);
        facts.push(("mechanism", EvidenceValue::Tag(r.mechanism.as_tag())));
        facts.push(("outcome", EvidenceValue::Tag(r.outcome.as_tag())));
        Finding {
            rule: self.id(),
            subject: Subject::Symbol(key),
            participants: participants(index, v, &v.dead),
            facts,
            gaps: r.gaps.clone(),
            ..Finding::unscored()
        }
    }
}
