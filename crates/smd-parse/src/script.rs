//! The event-driven (push) Clausewitz parser (doc 06 § Two-layer parser).
//!
//! Iterative: nesting is a counter plus a `Vec` of open-block offsets, never recursion on
//! input structure, so no input can overflow the stack (`catch_unwind` cannot catch that).
//! Nesting beyond [`DEPTH_MAX`] is skipped with a diagnostic; everything outside the
//! skipped block still reaches the sink. Every recovery marks the parse partial.

use smd_core::{DiagSeverity, RawDiagnostic, Span};

use crate::lexer::{Lexer, Op, TokKind, Token};

/// Deepest nesting that is parsed (doc 06 § Iterative, with an explicit depth cap).
pub const DEPTH_MAX: usize = 256;

/// Diagnostics kept per file; the rest are counted into one summary diagnostic.
pub const DIAGS_MAX: usize = 32;

/// Diagnostic codes this parser emits.
pub mod codes {
    /// A block was still open at end of input; it was closed there.
    pub const UNBALANCED: &str = "clausewitz.unbalanced_brace";
    /// A `}` with no open block; skipped.
    pub const STRAY_CLOSE: &str = "clausewitz.stray_close";
    /// Nesting beyond the cap; the block was skipped.
    pub const DEPTH_LIMIT: &str = "clausewitz.depth_limit";
    /// An operator with no key, or a key and operator with no value.
    pub const MALFORMED: &str = "clausewitz.malformed_statement";
    /// A quoted string ran to end of input.
    pub const UNTERMINATED: &str = "clausewitz.unterminated_string";
    /// The file is not UTF-8 and was decoded as Windows-1252.
    pub const ENCODING: &str = "clausewitz.encoding_fallback";
    /// A key decoded with a replacement character.
    pub const BAD_KEY: &str = "clausewitz.replacement_in_key";
    /// More diagnostics than [`DIAGS_MAX`](super::DIAGS_MAX) in one file.
    pub const TRUNCATED: &str = "clausewitz.diagnostics_truncated";
}

/// Receives parse events. Every event method defaults to doing nothing; recoveries go to
/// the sink's [`Diags`].
///
/// `depth` is the depth of the key: a top-level `key = { … }` opens at depth 0, and its
/// children are at depth 1. `block_close` carries the same depth as its `block_open`.
pub trait ScriptSink {
    /// Where recoveries are recorded.
    fn diags(&mut self) -> &mut Diags;
    /// `key = {`, `key = rgb {`, or an anonymous `{` (`key` is `None`).
    fn block_open(&mut self, _key: Option<&Token>, _depth: u16, _open: &Token) {}
    /// The matching `}`; at end-of-input recovery a zero-length token at EOF.
    fn block_close(&mut self, _depth: u16, _close: &Token) {}
    /// `key op value`.
    fn assign(&mut self, _key: &Token, _op: Op, _value: &Token, _depth: u16) {}
    /// A value with no key (`{ a b c }` list items).
    fn bare(&mut self, _value: &Token, _depth: u16) {}
}

/// Collects a file's diagnostics, capped, and whether any recovery happened.
#[derive(Debug, Default)]
pub struct Diags {
    /// Kept diagnostics, in input order.
    pub list: Vec<RawDiagnostic>,
    /// Some recovery happened: the parse is `Partial`.
    pub partial: bool,
    dropped: u32,
}

impl Diags {
    /// Record a diagnostic; `recovery` marks the parse partial.
    pub fn push(&mut self, code: &str, at: Span, detail: String, recovery: bool) {
        self.partial |= recovery;
        if self.list.len() < DIAGS_MAX {
            let severity = DiagSeverity::Warn;
            self.list.push(RawDiagnostic {
                span: at,
                code: code.into(),
                severity,
                detail: detail.into(),
            });
        } else {
            self.dropped += 1;
        }
    }

    /// The kept list, with a summary if any were dropped.
    #[must_use]
    pub fn finish(mut self) -> (Vec<RawDiagnostic>, bool) {
        if self.dropped > 0 {
            let detail = format!("{} further diagnostics not listed", self.dropped);
            let span = Span::default();
            let severity = DiagSeverity::Warn;
            self.list.push(RawDiagnostic {
                span,
                code: codes::TRUNCATED.into(),
                severity,
                detail: detail.into(),
            });
        }
        (self.list, self.partial)
    }
}

/// The span of a token.
#[must_use]
pub fn span(t: &Token) -> Span {
    Span { offset: t.start, len: t.end - t.start, line: t.line }
}

fn depth_of(open: &[u32]) -> u16 {
    u16::try_from(open.len()).unwrap_or(u16::MAX)
}

/// Parse `bytes` from `start`, pushing events and recoveries into `sink`. `cancelled` is
/// polled once per top-level statement.
pub fn parse(bytes: &[u8], start: usize, sink: &mut dyn ScriptSink, cancelled: &dyn Fn() -> bool) {
    let mut lx = Lexer::new(bytes, start);
    let mut open: Vec<u32> = Vec::new();
    loop {
        if open.is_empty() && cancelled() {
            // The caller discards a cancelled parse; partial keeps it from reading as whole.
            sink.diags().partial = true;
            return;
        }
        let t = lx.next_token();
        match t.kind {
            TokKind::Eof => break,
            TokKind::Close => match open.pop() {
                Some(_) => sink.block_close(depth_of(&open), &t),
                None => sink.diags().push(
                    codes::STRAY_CLOSE,
                    span(&t),
                    "`}` with no open block".into(),
                    true,
                ),
            },
            TokKind::Open => open_block(&mut lx, &mut open, (None, &t), sink),
            TokKind::Op(_) => {
                sink.diags().push(codes::MALFORMED, span(&t), "operator with no key".into(), true)
            }
            TokKind::Word | TokKind::Quoted => statement(&mut lx, &mut open, &t, sink),
        }
    }
    if lx.unterminated {
        let at = Span { offset: u32::try_from(bytes.len()).unwrap_or(u32::MAX), len: 0, line: 0 };
        sink.diags().push(
            codes::UNTERMINATED,
            at,
            "quoted string runs to end of file".into(),
            true,
        );
    }
    close_at_eof(&mut lx, &mut open, sink);
}

fn close_at_eof(lx: &mut Lexer<'_>, open: &mut Vec<u32>, sink: &mut dyn ScriptSink) {
    if open.is_empty() {
        return;
    }
    let eof = lx.peek();
    let detail = format!("{} block(s) open at end of file; closed there", open.len());
    sink.diags().push(codes::UNBALANCED, span(&eof), detail, true);
    while open.pop().is_some() {
        sink.block_close(depth_of(open), &eof);
    }
}

/// A key: `key op value`, `key op {`, `key op word {`, or a bare value.
fn statement(lx: &mut Lexer<'_>, open: &mut Vec<u32>, key: &Token, sink: &mut dyn ScriptSink) {
    let TokKind::Op(op) = lx.peek().kind else {
        sink.bare(key, depth_of(open));
        return;
    };
    lx.next_token();
    let v = lx.peek();
    match v.kind {
        TokKind::Open => {
            lx.next_token();
            open_block(lx, open, (Some(key), &v), sink);
        }
        TokKind::Word | TokKind::Quoted => {
            lx.next_token();
            if v.kind == TokKind::Word && lx.peek().kind == TokKind::Open {
                // `color = rgb { … }`: a prefixed block keyed by `key`.
                let brace = lx.next_token();
                open_block(lx, open, (Some(key), &brace), sink);
            } else {
                sink.assign(key, op, &v, depth_of(open));
            }
        }
        // Leave the `}` or EOF for the main loop.
        _ => sink.diags().push(
            codes::MALFORMED,
            span(key),
            "key and operator with no value".into(),
            true,
        ),
    }
}

/// Open a block, or skip it whole if it would exceed [`DEPTH_MAX`].
fn open_block(
    lx: &mut Lexer<'_>,
    open: &mut Vec<u32>,
    (key, brace): (Option<&Token>, &Token),
    sink: &mut dyn ScriptSink,
) {
    if open.len() < DEPTH_MAX {
        sink.block_open(key, depth_of(open), brace);
        open.push(brace.start);
        return;
    }
    let detail = format!("nesting deeper than {DEPTH_MAX}; block skipped");
    sink.diags().push(codes::DEPTH_LIMIT, span(brace), detail, true);
    let mut level = 1usize;
    while level > 0 {
        match lx.next_token().kind {
            TokKind::Open => level += 1,
            TokKind::Close => level -= 1,
            TokKind::Eof => return,
            _ => {}
        }
    }
}
