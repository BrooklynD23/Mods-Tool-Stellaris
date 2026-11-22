//! `core.loc_collision` — one localisation key in one language from two or more live
//! entries (doc 08 catalogue).
//!
//! Split by value: when every live entry carries the same value, `agreement` is
//! `all_identical` whatever the resolution, and policy discounts it as benign — whichever
//! copy wins, the player reads the same text. Otherwise the resolution's agreement stands.
//! With `localisation/replace/`-style entries the tier decides first (H3); with an
//! incomplete file at or above the winner's tier no winner is stated.

use smd_core::{
    Agreement, Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, LangId, Outcome,
    Participant, Role, RuleId, Subject, SymbolId,
};
use smd_index::{LocVerdict, ScanIndex};

use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct LocCollision;

impl LocCollision {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(LocCollision::CASES);

impl Rule for LocCollision {
    fn id(&self) -> RuleId {
        RuleId("core.loc_collision")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::LOCALISATION | Capabilities::OVERLAY
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
        for ((lang, key), sites) in ctx.index.loc().multi_defined() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            if sites.iter().filter(|d| ctx.index.derived().contends(d.file_slot)).count() < 2 {
                continue;
            }
            let v = ctx.index.resolve_loc(lang, key);
            if v.live.len() >= 2 {
                out.push(self.finding(ctx.index, (lang, key), &v));
            }
        }
        out
    }
}

/// Roles under doc 07 R1: `Shadowed` only for a stated survivor under `SingleWinner`.
fn participants(index: &ScanIndex, v: &LocVerdict) -> Vec<Participant> {
    let r = &v.resolution;
    let mut out: Vec<Participant> = v
        .live
        .iter()
        .map(|d| {
            let is_winner =
                r.winner.is_some_and(|w| w.slot == Some(d.file_slot) && w.source == d.site.source);
            let role = if is_winner {
                Role::Winner
            } else if r.outcome == Outcome::SingleWinner
                && index.derived().is_surviving(d.file_slot)
            {
                Role::Shadowed
            } else {
                Role::Contributor
            };
            Participant { site: d.site, role }
        })
        .collect();
    out.sort_by_key(|p| p.site.sort_key());
    out
}

impl LocCollision {
    fn finding(
        &self,
        index: &ScanIndex,
        (lang, key): (LangId, SymbolId),
        v: &LocVerdict,
    ) -> Finding {
        let r = &v.resolution;
        let same_value = v.live.iter().all(|d| d.value_hash == v.live[0].value_hash);
        let agreement = if same_value { Agreement::AllIdentical } else { r.agreement };
        let mut facts = EvidenceBag::new();
        facts.push(("agreement", EvidenceValue::Tag(agreement.as_tag())));
        facts.push(("mechanism", EvidenceValue::Tag(r.mechanism.as_tag())));
        let merge = index.class_of(v.live[0].site.path).merge;
        facts.push(("merge", EvidenceValue::Tag(merge.as_tag())));
        Finding {
            rule: self.id(),
            subject: Subject::LocKey { lang, key },
            participants: participants(index, v),
            facts,
            gaps: r.gaps.clone(),
            ..Finding::unscored()
        }
    }
}
