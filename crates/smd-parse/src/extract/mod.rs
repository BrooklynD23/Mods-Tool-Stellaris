//! From events to facts (doc 06 § From events to facts). Which key is a definition is
//! profile data — the path's [`DirClass`] — never a game name in code.

mod digest;
mod event;

pub use digest::{Split, digests, hash16};
pub use event::EventExtractor;

use smd_core::{DomainId, FactPayload, RawFact, Span};
use smd_profiles::DirClass;

use crate::lexer::{Op, TokKind, Token};
use crate::script::{Diags, ScriptSink, codes, span};
use crate::text::{Encoding, token_text};

/// What every extractor shares: the bytes, their encoding, and where facts go.
pub(crate) struct Out<'a> {
    pub bytes: &'a [u8],
    pub enc: Encoding,
    pub facts: Vec<RawFact>,
    pub diags: Diags,
}

impl Out<'_> {
    /// A key's text; a replacement character in it marks the parse partial.
    pub fn key(&mut self, t: &Token) -> Box<str> {
        let text = token_text(self.bytes, t, self.enc);
        if text.contains('\u{FFFD}') {
            let detail = "key holds a byte that does not decode".to_owned();
            self.diags.push(codes::BAD_KEY, span(t), detail, true);
        }
        text.into()
    }
}

struct Open {
    symbol: Box<str>,
    key: Token,
    body_start: usize,
}

/// `depth_key`: every key at the class's `level` is a definition. Above level 0 the
/// symbol is the enclosing keys joined with `.` (`NGameplay.MAX_X`), so equal names in
/// different groups stay distinct. `@`-variables are file-local and not definitions.
pub struct DepthKey<'a> {
    pub(crate) out: Out<'a>,
    level: u16,
    domain: DomainId,
    children: &'a [Box<str>],
    path: Vec<Option<Box<str>>>,
    cur: Option<Open>,
}

impl<'a> DepthKey<'a> {
    pub(crate) fn new(out: Out<'a>, class: &'a DirClass, domain: DomainId) -> DepthKey<'a> {
        let children = &*class.additive_children;
        let level = u16::from(class.level);
        DepthKey { out, level, domain, children, path: Vec::new(), cur: None }
    }

    /// The qualified symbol for `key` at the extraction level, if every ancestor is named.
    fn symbol(&mut self, key: &Token) -> Option<Box<str>> {
        let name = self.out.key(key);
        if name.starts_with('@') || name.is_empty() {
            return None;
        }
        let mut parts: Vec<&str> = Vec::with_capacity(self.path.len() + 1);
        for p in &self.path {
            parts.push(p.as_deref()?);
        }
        parts.push(&name);
        Some(if parts.len() == 1 { name } else { parts.join(".").into() })
    }

    fn emit(&mut self, symbol: Box<str>, at: Span, (body, split): ([u8; 16], Option<Split>)) {
        let domain = self.domain;
        let def = FactPayload::Definition { domain, symbol: symbol.clone(), body_hash: body };
        self.out.facts.push(RawFact { span: at, payload: def });
        if let Some((residual_hash, additive_count)) = split {
            let payload =
                FactPayload::AdditiveSplit { domain, symbol, residual_hash, additive_count };
            self.out.facts.push(RawFact { span: at, payload });
        }
    }
}

fn joined(a: &Token, b: &Token) -> Span {
    Span { offset: a.start, len: b.end.saturating_sub(a.start), line: a.line }
}

impl ScriptSink for DepthKey<'_> {
    fn diags(&mut self) -> &mut Diags {
        &mut self.out.diags
    }

    fn block_open(&mut self, key: Option<&Token>, depth: u16, brace: &Token) {
        if depth < self.level {
            let name = key.map(|k| self.out.key(k));
            self.path.push(name);
        } else if depth == self.level {
            let found = key.and_then(|k| Some((*k, self.symbol(k)?)));
            self.cur =
                found.map(|(key, symbol)| Open { symbol, key, body_start: brace.end as usize });
        }
    }

    fn block_close(&mut self, depth: u16, close: &Token) {
        if depth < self.level {
            self.path.pop();
        } else if depth == self.level {
            if let Some(o) = self.cur.take() {
                let range = (o.body_start, close.start as usize);
                let d = digests(self.out.bytes, range, self.out.enc, self.children);
                self.emit(o.symbol, joined(&o.key, close), d);
            }
        }
    }

    fn assign(&mut self, key: &Token, op: Op, value: &Token, depth: u16) {
        // A comparison (`key > 5`) at the level is a condition, not a definition.
        if depth != self.level || op != Op::Eq || key.kind == TokKind::Eof {
            return;
        }
        if let Some(symbol) = self.symbol(key) {
            let body = hash16(&self.out.bytes[value.start as usize..value.end as usize]);
            let split = (!self.children.is_empty()).then_some((body, 0));
            self.emit(symbol, joined(key, value), (body, split));
        }
    }
}
