//! `core.file_overwrite` — two or more sources contribute the same path.
//!
//! Reads Stage A only. It names no directory and no game: whether a shared path is a
//! conflict at all is the profile's `same_path`, which arrives through the index.

use smd_core::{
    Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, Participant, PathId, Role,
    RuleId, Site, Span, Subject,
};
use smd_index::{PathVerdict, ScanIndex};

use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct FileOverwrite;

impl FileOverwrite {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(FileOverwrite::CASES);

impl Rule for FileOverwrite {
    fn id(&self) -> RuleId {
        RuleId("core.file_overwrite")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::OVERLAY
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[
            ("agreement", EvidenceType::Tag("Agreement")),
            ("mechanism", EvidenceType::Tag("Mechanism")),
            ("same_path", EvidenceType::Tag("SamePath")),
        ]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for p in ctx.index.overlay().multi_contributor_paths() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let verdict = ctx.index.path_verdict(p);
            // Stage A participants: stated or possible survivors, plus same-path losers.
            // Inactive sources and `replace_path` drops are never among them.
            if verdict.live.len() < 2 {
                continue;
            }
            out.push(self.finding(ctx.index, p, &verdict));
        }
        out
    }
}

impl FileOverwrite {
    fn finding(&self, index: &ScanIndex, p: PathId, v: &PathVerdict) -> Finding {
        let res = &v.resolution;
        let mut facts = EvidenceBag::new();
        facts.push(("agreement", EvidenceValue::Tag(res.agreement.as_tag())));
        facts.push(("mechanism", EvidenceValue::Tag(res.mechanism.as_tag())));
        facts.push(("same_path", EvidenceValue::Tag(index.same_path_of(p).as_tag())));
        Finding {
            rule: self.id(),
            subject: Subject::Path(p),
            participants: participants(index, p, v),
            facts,
            gaps: res.gaps.clone(),
            ..Finding::unscored()
        }
    }
}

/// Winner / Shadowed / Contributor from the Stage A resolution (invariant R1: `Shadowed`
/// only under `SingleWinner`).
fn participants(index: &ScanIndex, p: PathId, v: &PathVerdict) -> Vec<Participant> {
    let res = &v.resolution;
    let mut out: Vec<Participant> = v
        .live
        .iter()
        .map(|c| {
            let role = if res.winner == Some(*c) {
                Role::Winner
            } else if res.outcome.permits_shadowed_participants() {
                Role::Shadowed
            } else {
                Role::Contributor
            };
            let len = index
                .overlay()
                .contributions(p)
                .iter()
                .find(|x| x.source == c.source)
                .and_then(|x| x.blob)
                .map_or(0, |b| u32::try_from(b.len).unwrap_or(u32::MAX));
            Participant {
                site: Site { source: c.source, path: p, span: Span { offset: 0, len, line: 1 } },
                role,
            }
        })
        .collect();
    // `live` is ascending by source index already; load order is never compared here.
    out.sort_by_key(|x| x.site.source.index());
    out
}
