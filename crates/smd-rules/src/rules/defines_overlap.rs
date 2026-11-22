//! `pdx.defines_overlap` — one define `group.key` with two or more live sites.
//!
//! Fires on the *key*, never on file overlap: two mods each shipping a one-key defines
//! file with different keys is a non-event (profile, `common/defines`). The group resolves
//! through Stages C and D, so the finding names the winning file or, with gaps, none.

use smd_core::{
    Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, RuleId, Subject, SymbolKey,
};
use smd_index::{ScanIndex, SymbolVerdict};

use super::symbols::participants;
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct DefinesOverlap;

impl DefinesOverlap {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(DefinesOverlap::CASES);

impl Rule for DefinesOverlap {
    fn id(&self) -> RuleId {
        RuleId("pdx.defines_overlap")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DEFINITIONS | Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[
            ("agreement", EvidenceType::Tag("Agreement")),
            ("mechanism", EvidenceType::Tag("Mechanism")),
            ("merge", EvidenceType::Tag("MergeMode")),
        ]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (key, sites) in ctx.index.defines().multi_defined() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            if sites.iter().filter(|d| ctx.index.derived().contends(d.file_slot)).count() < 2 {
                continue;
            }
            let v = ctx.index.resolve_define(*key);
            if v.live.len() >= 2 {
                out.push(self.finding(ctx.index, *key, &v));
            }
        }
        out
    }
}

impl DefinesOverlap {
    fn finding(&self, index: &ScanIndex, key: SymbolKey, v: &SymbolVerdict) -> Finding {
        let r = &v.resolution;
        let mut facts = EvidenceBag::new();
        facts.push(("agreement", EvidenceValue::Tag(r.agreement.as_tag())));
        facts.push(("mechanism", EvidenceValue::Tag(r.mechanism.as_tag())));
        let merge = index.class_of(v.live[0].site.path).merge;
        facts.push(("merge", EvidenceValue::Tag(merge.as_tag())));
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
