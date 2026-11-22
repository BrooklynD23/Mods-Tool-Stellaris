//! `pdx.replace_path_shadow` — a `replace_path` declaration drops another mod's files.
//!
//! Dropping vanilla's (or a DLC's) files is what `replace_path` is for, so only other
//! mods' files count. A file the declaration stated-drops is `Shadowed` when its own path
//! resolution permits it (R1); a file it only *may* drop (scope, order or activation
//! unknown) is a `Contributor` and its gaps ride on the finding. A content-incomplete
//! other mod may hold more files under the prefix: `ParseIncomplete`. Every definition in
//! a dropped file has already left its key group (Stage B), so the symbol-level effect of
//! the drop shows up in `core.symbol_collision` / `core.symbol_deleted`, not here.

use smallvec::SmallVec;
use smd_core::{
    Activation, Capabilities, EvidenceBag, EvidenceType, EvidenceValue, Finding, Participant,
    ResolutionGap, Role, RuleId, Site, SourceRole, Span, Subject,
};
use smd_index::{ReplaceDecl, ReplaceHit, ScanIndex};

use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

mod cases;

/// See the module docs.
pub struct ReplacePathShadow;

impl ReplacePathShadow {
    const CASES: &'static [RuleCase] = cases::CASES;
}

const _: () = RuleCase::assert_covers_all_kinds(ReplacePathShadow::CASES);

impl Rule for ReplacePathShadow {
    fn id(&self) -> RuleId {
        RuleId("pdx.replace_path_shadow")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::OVERLAY | Capabilities::DESCRIPTORS
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("files", EvidenceType::Int)]
    }

    fn cases(&self) -> &[RuleCase] {
        Self::CASES
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for d in ctx.index.replace_decls() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let hits = ctx.index.replace_hits(d);
            if let Some(f) = self.finding(ctx.index, d, &hits) {
                out.push(f);
            }
        }
        out
    }
}

/// Whether a non-`Inactive` other mod was not fully listed or read.
fn others_incomplete(index: &ScanIndex, d: &ReplaceDecl) -> bool {
    index.content_incomplete().iter().any(|s| {
        *s != d.declarer
            && index.sources().get(*s).is_some_and(|m| {
                m.activation != Activation::Inactive
                    && !matches!(m.role, SourceRole::Vanilla | SourceRole::Dlc)
            })
    })
}

fn role(index: &ScanIndex, h: &ReplaceHit) -> Role {
    let permits = index.resolve_path(h.contender.path).outcome.permits_shadowed_participants();
    if h.stated && permits { Role::Shadowed } else { Role::Contributor }
}

impl ReplacePathShadow {
    fn finding(&self, index: &ScanIndex, d: &ReplaceDecl, hits: &[ReplaceHit]) -> Option<Finding> {
        let prefix = index.path_id(&d.prefix)?;
        if hits.is_empty() {
            return None;
        }
        let mut gaps: SmallVec<[ResolutionGap; 1]> =
            hits.iter().flat_map(|h| h.gaps.iter().copied()).collect();
        if others_incomplete(index, d) {
            gaps.push(ResolutionGap::ParseIncomplete);
        }
        gaps.sort();
        gaps.dedup();
        let declarer = Site { source: d.declarer, path: prefix, span: Span::default() };
        let mut participants = vec![Participant { site: declarer, role: Role::Contributor }];
        participants.extend(hits.iter().map(|h| Participant {
            site: Site {
                source: h.contender.source,
                path: h.contender.path,
                span: Span::default(),
            },
            role: role(index, h),
        }));
        participants.sort_by_key(|p| p.site.sort_key());
        let mut facts = EvidenceBag::new();
        facts.push(("files", EvidenceValue::Int(i64::try_from(hits.len()).unwrap_or(i64::MAX))));
        Some(Finding {
            rule: self.id(),
            subject: Subject::Directory { declarer: d.declarer, prefix },
            participants,
            facts,
            gaps,
            ..Finding::unscored()
        })
    }
}
