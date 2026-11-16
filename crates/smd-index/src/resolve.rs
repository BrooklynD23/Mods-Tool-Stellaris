//! `resolve_finding` — ids to text, at the boundary (doc 07 § Resolving findings).

use smallvec::SmallVec;
use smd_core::{
    Diagnostic, EvidenceValue, Finding, ModKey, Participant, ResolvedEvidence, ResolvedFinding,
    ResolvedParticipant, SourceId, Subject, SubjectPart,
};

use crate::index::ScanIndex;

impl ScanIndex {
    /// Turn a finding into text only: no id survives except the retained `subject`.
    #[must_use]
    pub fn resolve_finding(&self, f: &Finding) -> ResolvedFinding {
        let (subject_text, subject_parts) = self.subject_text(&f.subject);
        ResolvedFinding {
            rule: f.rule,
            subject: f.subject.clone(),
            subject_text,
            subject_parts,
            participants: f.participants.iter().map(|p| self.resolve_participant(p)).collect(),
            facts: f.facts.iter().map(|(k, v)| (*k, self.resolve_evidence(*v))).collect(),
            certainty: f.certainty,
            gaps: f.gaps.clone(),
            score: f.score,
            severity: f.severity,
        }
    }

    /// Path text, or the unhandled marker for an id this scan never issued.
    fn path_text_or_marker(&self, p: smd_core::PathId) -> &str {
        self.paths.try_get(p.0).unwrap_or(Diagnostic::UNHANDLED)
    }

    fn mod_key(&self, s: SourceId) -> ModKey {
        self.sources.get(s).map_or_else(|| ModKey("<unknown>".into()), |m| m.key.clone())
    }

    fn resolve_participant(&self, p: &Participant) -> ResolvedParticipant {
        let name = self.sources.get(p.site.source).map_or("<unknown>", |m| &*m.name);
        ResolvedParticipant {
            mod_key: self.mod_key(p.site.source),
            mod_name: name.into(),
            path: self.path_text_or_marker(p.site.path).into(),
            span: p.site.span,
            role: p.role,
        }
    }

    fn resolve_evidence(&self, v: EvidenceValue) -> ResolvedEvidence {
        match v {
            EvidenceValue::Int(n) => ResolvedEvidence::Int(n),
            EvidenceValue::Bool(b) => ResolvedEvidence::Bool(b),
            EvidenceValue::Tag(t) => ResolvedEvidence::Tag(t),
            EvidenceValue::Sym(s) => ResolvedEvidence::Text(self.symbols.get(s.0).into()),
            EvidenceValue::Path(p) => ResolvedEvidence::Text(self.path_text_or_marker(p).into()),
            EvidenceValue::Src(s) => ResolvedEvidence::Text(self.mod_key(s).as_str().into()),
            _ => ResolvedEvidence::Text(Diagnostic::UNHANDLED.into()),
        }
    }

    fn subject_text(&self, s: &Subject) -> (Box<str>, Vec<SubjectPart>) {
        let text = |t: &str| SubjectPart::Text(t.into());
        match s {
            Subject::Path(p) => {
                let t = self.path_text_or_marker(*p);
                (t.into(), vec![text(t)])
            }
            Subject::Mod(m) => {
                let k = self.mod_key(*m);
                (k.as_str().into(), vec![text(k.as_str())])
            }
            Subject::ModPair(a, b) => {
                let (a, b) = (self.mod_key(*a), self.mod_key(*b));
                (format!("{a} + {b}").into(), vec![text(a.as_str()), text(b.as_str())])
            }
            Subject::Directory { declarer, prefix } => {
                let (k, p) = (self.mod_key(*declarer), self.path_text_or_marker(*prefix));
                (format!("{k}: {p}").into(), vec![text(k.as_str()), text(p)])
            }
            Subject::Symbol(k) => {
                let name = self.symbols.get(k.symbol.0);
                (name.into(), vec![text(&k.domain.0.to_string()), text(name)])
            }
            other => (
                format!("{} ({other:?})", Diagnostic::UNHANDLED).into(),
                SmallVec::<[SubjectPart; 1]>::new().into_vec(),
            ),
        }
    }
}
