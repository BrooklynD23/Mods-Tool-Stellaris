//! The `pdx_event` extractor: `namespace = x` at depth 0, and `id = x.N` directly inside
//! each depth-0 block (doc 06 § From events to facts).

use smd_core::{EventOrdinal, FactPayload, RawFact};

use super::Out;
use crate::lexer::{Op, Token};
use crate::script::{Diags, ScriptSink, span};

/// See the module docs. Emits `Namespace` and `EventId` facts, no `Definition`.
pub struct EventExtractor<'a> {
    pub(crate) out: Out<'a>,
}

impl<'a> EventExtractor<'a> {
    pub(crate) fn new(out: Out<'a>) -> EventExtractor<'a> {
        EventExtractor { out }
    }
}

/// `ns.100` → `(ns, Num(100))`; `ns.setup` → `(ns, Name("setup"))`. Event ids need not be
/// numeric, and are never coerced to a number.
fn split_id(id: &str) -> Option<(Box<str>, EventOrdinal<Box<str>>)> {
    let (ns, ord) = id.rsplit_once('.')?;
    if ns.is_empty() || ord.is_empty() {
        return None;
    }
    let ordinal = match ord.parse::<u32>() {
        Ok(n) if !ord.starts_with('0') || ord == "0" => EventOrdinal::Num(n),
        _ => EventOrdinal::Name(ord.into()),
    };
    Some((ns.into(), ordinal))
}

impl ScriptSink for EventExtractor<'_> {
    fn diags(&mut self) -> &mut Diags {
        &mut self.out.diags
    }

    fn assign(&mut self, key: &Token, op: Op, value: &Token, depth: u16) {
        if op != Op::Eq || depth > 1 {
            return;
        }
        let k = self.out.key(key);
        let payload = match (depth, &*k) {
            (0, "namespace") => FactPayload::Namespace { symbol: self.out.key(value) },
            (1, "id") => match split_id(&self.out.key(value)) {
                Some((namespace, ordinal)) => FactPayload::EventId { namespace, ordinal },
                None => return,
            },
            _ => return,
        };
        self.out.facts.push(RawFact { span: span(value), payload });
    }
}
