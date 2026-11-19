//! `meta.descriptor_mismatch` — the launcher `*.mod` and the content directory's
//! `descriptor.mod` disagree on `name`, `version`, `remote_file_id` or
//! `supported_version`. One finding per differing field.
//!
//! A field is compared only when both copies set it: the launcher routinely adds
//! `remote_file_id` the author never wrote, and an absent value is no claim. No inner copy
//! (not read, unreadable, or not parsed cleanly) produces nothing.
//!
//! Untested: no cases yet (testing deferred by the owner).

use smd_core::{
    Capabilities, Descriptor, EvidenceBag, EvidenceType, EvidenceValue, Finding, Role, RuleId,
    SourceId, Subject,
};
use smd_index::{ScanIndex, pattern_text};

use super::playset::{STATED, maybe_loaded, mod_site, text};
use crate::case::RuleCase;
use crate::rule::{Rule, RuleCtx};

/// See the module docs.
pub struct MetaDescriptorMismatch;

/// `(field tag, launcher value, inner value)` for each field set in both and different.
fn differences(a: &Descriptor, b: &Descriptor) -> Vec<(&'static str, String, String)> {
    let both = |x: Option<String>, y: Option<String>| x.zip(y).filter(|(x, y)| x != y);
    let fields = [
        ("name", both(Some(a.name.to_string()), Some(b.name.to_string()))),
        (
            "version",
            both(a.version.as_deref().map(Into::into), b.version.as_deref().map(Into::into)),
        ),
        (
            "remote_file_id",
            both(
                a.remote_file_id.as_deref().map(Into::into),
                b.remote_file_id.as_deref().map(Into::into),
            ),
        ),
        (
            "supported_version",
            both(
                a.supported_version.as_ref().map(pattern_text),
                b.supported_version.as_ref().map(pattern_text),
            ),
        ),
    ];
    // An empty name is "unset", not a value.
    fields
        .into_iter()
        .filter_map(|(f, d)| d.map(|(x, y)| (f, x, y)))
        .filter(|(_, x, y)| !x.is_empty() && !y.is_empty())
        .collect()
}

impl Rule for MetaDescriptorMismatch {
    fn id(&self) -> RuleId {
        RuleId("meta.descriptor_mismatch")
    }

    fn needs(&self) -> Capabilities {
        Capabilities::DESCRIPTORS
    }

    fn evidence(&self) -> &'static [(&'static str, EvidenceType)] {
        &[
            ("field", EvidenceType::Tag("DescriptorField")),
            ("inner", EvidenceType::Sym),
            ("launcher", EvidenceType::Sym),
        ]
    }

    fn cases(&self) -> &[RuleCase] {
        &[]
    }

    fn run(&self, ctx: &RuleCtx<'_>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (s, _) in ctx.index.sources().iter() {
            if ctx.cancel.is_cancelled() {
                return out;
            }
            let Some(d) = ctx.index.meta().descriptors_of(s) else { continue };
            let Some(inner) = &d.inner else { continue };
            if !maybe_loaded(ctx.index, s) {
                continue;
            }
            for (field, launcher, inner) in differences(&d.declared, inner) {
                out.push(self.finding(ctx.index, s, field, (&launcher, &inner)));
            }
        }
        out
    }
}

impl MetaDescriptorMismatch {
    fn finding(
        &self,
        index: &ScanIndex,
        s: SourceId,
        field: &'static str,
        v: (&str, &str),
    ) -> Finding {
        let mut facts = EvidenceBag::new();
        facts.push(("field", EvidenceValue::Tag(field)));
        facts.push(("inner", text(index, v.1)));
        facts.push(("launcher", text(index, v.0)));
        Finding {
            rule: self.id(),
            subject: Subject::Mod(s),
            participants: vec![mod_site(s, Role::Contributor)],
            facts,
            certainty: STATED,
            ..Finding::unscored()
        }
    }
}
