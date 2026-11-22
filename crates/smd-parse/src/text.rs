//! Encoding (doc 06 § Additional realities): strip a BOM, try UTF-8, else decode
//! Windows-1252 lossily. Offsets never move: decoding is per token, on demand.

use std::borrow::Cow;

use crate::lexer::{TokKind, Token};

/// How a file's bytes decode.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Encoding {
    /// Valid UTF-8 after the BOM.
    Utf8,
    /// Not UTF-8: decoded as Windows-1252.
    Windows1252,
}

/// Where lexing starts (past a UTF-8 BOM) and how the rest decodes.
#[must_use]
pub fn detect(bytes: &[u8]) -> (usize, Encoding) {
    let start = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let enc = if std::str::from_utf8(&bytes[start..]).is_ok() {
        Encoding::Utf8
    } else {
        Encoding::Windows1252
    };
    (start, enc)
}

/// 0x80..=0x9F in Windows-1252; `\u{FFFD}` for the five undefined bytes.
const CP1252_HIGH: [char; 32] = [
    '€', '\u{FFFD}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{FFFD}', 'Ž',
    '\u{FFFD}', '\u{FFFD}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{FFFD}',
    'ž', 'Ÿ',
];

fn cp1252(b: u8) -> char {
    match b {
        0x80..=0x9F => CP1252_HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

/// The text of a token, quotes stripped from a quoted one.
#[must_use]
pub fn token_text<'a>(bytes: &'a [u8], t: &Token, enc: Encoding) -> Cow<'a, str> {
    let (mut s, mut e) = (t.start as usize, (t.end as usize).min(bytes.len()));
    if t.kind == TokKind::Quoted {
        s += 1;
        if e > s && bytes[e - 1] == b'"' {
            e -= 1;
        }
    }
    let raw = &bytes[s.min(e)..e];
    match enc {
        Encoding::Utf8 => String::from_utf8_lossy(raw),
        Encoding::Windows1252 => Cow::Owned(decode_1252(raw)),
    }
}

/// Decode bytes as Windows-1252.
#[must_use]
pub fn decode_1252(raw: &[u8]) -> String {
    raw.iter().map(|&b| cp1252(b)).collect()
}
