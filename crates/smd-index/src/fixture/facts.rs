//! Fixture files carrying event, namespace and define facts (M3 rule cases).

use smd_core::{EventOrdinal, FactPayload, ParseCompleteness, RawFact, SourceId, Span};
use smd_fs::NormPath;

use super::{FileShape, PlaysetBuilder, blob_of};

fn fact(payload: FactPayload<Box<str>>) -> RawFact {
    RawFact { span: Span::default(), payload }
}

/// `ns.100` → `(ns, Num(100))`; anything else after the last `.` is a name.
fn event_id(id: &str) -> FactPayload<Box<str>> {
    let (ns, ord) = id.rsplit_once('.').unwrap_or((id, ""));
    let ordinal = ord.parse().map_or_else(|_| EventOrdinal::Name(ord.into()), EventOrdinal::Num);
    FactPayload::EventId { namespace: ns.into(), ordinal }
}

impl PlaysetBuilder {
    fn parsed(&mut self, s: SourceId, path: &str, facts: Vec<RawFact>) -> &mut Self {
        let content = format!("{facts:?}");
        let shape = FileShape::of(&content).completeness(ParseCompleteness::Complete);
        self.file_with(s, path, shape);
        self.facts.push((s, NormPath::new(path).expect("fixture path must be valid"), facts));
        self
    }

    /// Ship a fully parsed events file: `namespace = ns` (when given), then each `id`.
    pub fn events(&mut self, s: SourceId, path: &str, ns: Option<&str>, ids: &[&str]) -> &mut Self {
        let head = ns.map(|n| fact(FactPayload::Namespace { symbol: n.into() }));
        let facts = head.into_iter().chain(ids.iter().map(|i| fact(event_id(i)))).collect();
        self.parsed(s, path, facts)
    }

    /// Ship a fully parsed defines file of `("Group.KEY", value)` pairs.
    pub fn defines(&mut self, s: SourceId, path: &str, keys: &[(&str, &str)]) -> &mut Self {
        let facts = keys
            .iter()
            .map(|(k, v)| {
                let (group, key) = k.split_once('.').unwrap_or(("", k));
                let value_hash = blob_of(v).hash;
                fact(FactPayload::DefineKey { group: group.into(), key: key.into(), value_hash })
            })
            .collect();
        self.parsed(s, path, facts)
    }
}
