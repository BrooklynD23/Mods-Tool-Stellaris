//! `pdx.event_id_dup` — one `namespace.ordinal` with two or more live sites.
//!
//! The group resolves through Stages C and D like a definition, so the finding names the
//! winning file (or none, with the gaps) and the axis that decided (H19). Two sites in one
//! mod count: a mod colliding with itself breaks the event just the same.
//!
//! `requires()` stays empty like every other rule's: the profile declares no feature tags
//! yet, and the engine skips a rule that requires one. An empty events table is silence.

use smd_core::{Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, RuleId, Subject};
use smd_index::{EventKey, ScanIndex, SymbolVerdict};

use super::symbols::participants;
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct EventIdDup;

impl EventIdDup {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(EventIdDup::CASES);

impl Rule for EventIdDup {
    fn id(&self) -> RuleId {
        RuleId("pdx.event_id_dup")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::EVENTS | Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("mechanism", EvidenceType::Tag("Mechanism")), ("merge", EvidenceType::Tag("MergeMode"))]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (key, sites) in ctx.index.events().multi_defined() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            if sites.iter().filter(|d| ctx.index.derived().contends(d.file_slot)).count() < 2 {
                continue;
            }
            let v = ctx.index.resolve_event(key);
            if v.live.len() >= 2 {
                out.push(self.finding(ctx.index, key, &v));
            }
        }
        out
    }
}

impl EventIdDup {
    fn finding(&self, index: &ScanIndex, key: &EventKey, v: &SymbolVerdict) -> Finding {
        let r = &v.resolution;
        let mut facts = EvidenceBag::new();
        facts.push(("mechanism", EvidenceValue::Tag(r.mechanism.as_tag())));
        let merge = index.class_of(v.live[0].site.path).merge;
        facts.push(("merge", EvidenceValue::Tag(merge.as_tag())));
        Finding {
            rule: self.id(),
            subject: Subject::Event { namespace: key.namespace, ordinal: key.ordinal.clone() },
            participants: participants(index, v, &v.live),
            facts,
            gaps: r.gaps.clone(),
            ..Finding::unscored()
        }
    }
}
