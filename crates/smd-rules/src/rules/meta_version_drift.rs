//! `meta.version_drift` — a mod's `supported_version` excludes the installed game version.
//!
//! Reads the declared descriptor (the copy that decides loading). Only a definite
//! [`Match::No`] fires: an absent, short or unparseable pattern, or an unreadable game
//! version, is no claim and produces nothing.
//!
//! Untested: no cases yet (testing deferred by the owner).

use smd_core::{Capabilities, EvidenceBag, EvidenceType, Finding, Match, Role, RuleId, Subject};
use smd_index::{pattern_text, version_text};

use super::playset::{STATED, maybe_loaded, mod_site, text};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

/// See the module docs.
pub struct MetaVersionDrift;

impl Rule for MetaVersionDrift {
    fn id(&self) -> RuleId {
        RuleId("meta.version_drift")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DESCRIPTORS
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[("game_version", EvidenceType::Sym), ("supported_version", EvidenceType::Sym)]
    }

    fn cases(&self) -> &[RuleCase] {
        &[]
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let meta = ctx.index.meta();
        let Some(gv) = &meta.game_version else { return Vec::new() };
        let mut out = Vec::new();
        for (s, _) in ctx.index.sources().iter() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let Some(pat) =
                meta.descriptors_of(s).and_then(|d| d.declared.supported_version.as_ref())
            else {
                continue;
            };
            if !maybe_loaded(ctx.index, s) || pat.matches(gv) != Match::No {
                continue;
            }
            let mut facts = EvidenceBag::new();
            facts.push(("game_version", text(ctx.index, &version_text(gv))));
            facts.push(("supported_version", text(ctx.index, &pattern_text(pat))));
            out.push(Finding {
                rule: self.id(),
                subject: Subject::Mod(s),
                participants: vec![mod_site(s, Role::Contributor)],
                facts,
                certainty: STATED,
                ..Finding::unscored()
            });
        }
        out
    }
}
