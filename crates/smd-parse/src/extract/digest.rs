//! Body hashing (doc 06 § Body hashing) and the `AdditiveSplit` residual (H4).
//!
//! A body is normalised by re-lexing its byte range and joining the tokens with one space:
//! comments and whitespace runs vanish, so reformatting is not a change. The residual is
//! the same token stream with every *direct* child named in `additive_children` removed —
//! key, operator, and value (a scalar, a block, or a prefixed block).

use crate::lexer::{Lexer, TokKind, Token};
use crate::text::{Encoding, token_text};

/// blake3 over `bytes`, truncated to 16 bytes.
#[must_use]
pub fn hash16(bytes: &[u8]) -> [u8; 16] {
    smd_fs::hash_reader(bytes, bytes.len() as u64).map_or([0; 16], |(b, _)| b.hash)
}

/// The additive split of one body: residual hash and how many children were removed.
pub type Split = ([u8; 16], u16);

struct Ctx<'a> {
    bytes: &'a [u8],
    enc: Encoding,
    children: &'a [Box<str>],
}

fn push(out: &mut Vec<u8>, bytes: &[u8], t: &Token) {
    if !out.is_empty() {
        out.push(b' ');
    }
    out.extend_from_slice(&bytes[t.start as usize..t.end as usize]);
}

fn additive_key(cx: &Ctx<'_>, t: &Token) -> bool {
    matches!(t.kind, TokKind::Word | TokKind::Quoted)
        && cx.children.iter().any(|c| **c == *token_text(cx.bytes, t, cx.enc))
}

/// Skip a value whose first token is `first`: a balanced block, `word {…}`, or one token.
fn skip_value(lx: &mut Lexer<'_>, first: Token) {
    let mut level = match first.kind {
        TokKind::Open => 1,
        TokKind::Word if lx.peek().kind == TokKind::Open => {
            lx.next_token();
            1
        }
        _ => 0,
    };
    while level > 0 {
        match lx.next_token().kind {
            TokKind::Open => level += 1,
            TokKind::Close => level -= 1,
            TokKind::Eof => return,
            _ => {}
        }
    }
}

/// The normalised-body hash of `bytes[start..end]` and, when `children` is non-empty, its
/// additive split.
#[must_use]
pub fn digests(
    bytes: &[u8],
    (start, end): (usize, usize),
    enc: Encoding,
    children: &[Box<str>],
) -> ([u8; 16], Option<Split>) {
    let body = &bytes[..end.min(bytes.len())];
    let mut full = Vec::new();
    let mut lx = Lexer::new(body, start);
    loop {
        let t = lx.next_token();
        if t.kind == TokKind::Eof {
            break;
        }
        push(&mut full, body, &t);
    }
    if children.is_empty() {
        return (hash16(&full), None);
    }
    let cx = Ctx { bytes: body, enc, children };
    (hash16(&full), Some(residual(&cx, start)))
}

fn residual(cx: &Ctx<'_>, start: usize) -> Split {
    let (mut out, mut removed, mut depth) = (Vec::new(), 0u16, 0usize);
    let mut lx = Lexer::new(cx.bytes, start);
    loop {
        let t = lx.next_token();
        match t.kind {
            TokKind::Eof => break,
            TokKind::Open => depth += 1,
            TokKind::Close => depth = depth.saturating_sub(1),
            _ => {}
        }
        if depth == 0 && additive_key(cx, &t) && matches!(lx.peek().kind, TokKind::Op(_)) {
            lx.next_token();
            let v = lx.next_token();
            skip_value(&mut lx, v);
            removed = removed.saturating_add(1);
            continue;
        }
        push(&mut out, cx.bytes, &t);
    }
    (hash16(&out), removed)
}
