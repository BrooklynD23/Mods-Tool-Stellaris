//! `locyml`: Paradox localisation (doc 06 § Localisation parsing). Not YAML — a real YAML
//! parser fails on it — but a line format:
//!
//! ```text
//! l_english:
//!  key:0 "value with \"escapes\", $VAR$ and £icon£"
//!  key_without_ordinal "value"   # a comment
//! ```
//!
//! Emits one `LocKey` per entry: the header's language (a position in the class's
//! `languages`), the key, the optional ordinal (absent is not zero), and a hash of the
//! value as written between its quotes. A header may repeat. A line the parser cannot read
//! is a diagnostic and makes the file `Partial`: its keys are unknown, not absent. The
//! same holds for entries under a language the profile does not list.

use smd_core::{FactPayload, FileKind, LangId, RawFact, Span};

use crate::extract::hash16;
use crate::registry::{FileParser, ParseInput, ParseOutput};
use crate::script::Diags;
use crate::text::{Encoding, detect};

/// Diagnostic codes.
pub mod codes {
    /// The file is not UTF-8 and was decoded as Windows-1252.
    pub const ENCODING: &str = "locyml.encoding_fallback";
    /// A line that is neither a header, an entry, a comment, nor blank.
    pub const MALFORMED: &str = "locyml.malformed_line";
    /// An entry before any `l_<lang>:` header.
    pub const NO_HEADER: &str = "locyml.entry_without_header";
    /// A header naming a language the profile does not list.
    pub const UNKNOWN_LANG: &str = "locyml.unknown_language";
    /// A key longer than [`KEY_MAX`](super::KEY_MAX) bytes.
    pub const KEY_TOO_LONG: &str = "locyml.key_too_long";
}

/// Longest key extracted, in bytes. Longer is a diagnostic, not a fact.
pub const KEY_MAX: usize = 1024;

/// Cancellation is polled once per this many lines.
const POLL_LINES: usize = 4096;

/// The Paradox localisation parser.
pub struct LocYmlParser;

/// Where the current header left us.
enum Lang {
    /// No header yet.
    None,
    /// A listed language.
    Known(LangId),
    /// A header naming an unlisted language: its entries are skipped.
    Unlisted,
}

/// One entry line, split.
struct Entry<'a> {
    key: &'a [u8],
    ordinal: Option<u16>,
    value: &'a [u8],
}

fn is_space(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\r'
}

fn trim_start(l: &[u8]) -> &[u8] {
    let n = l.iter().take_while(|b| is_space(**b)).count();
    &l[n..]
}

/// `l_<lang>:` followed only by blanks or a comment.
fn header(l: &[u8]) -> Option<&[u8]> {
    let rest = l.strip_prefix(b"l_")?;
    let colon = rest.iter().position(|b| *b == b':')?;
    let name = &rest[..colon];
    let tail = trim_start(&rest[colon + 1..]);
    let ok = !name.is_empty()
        && name.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_')
        && (tail.is_empty() || tail[0] == b'#');
    ok.then_some(name)
}

/// The value between the first `"` and the last unescaped `"` of the line.
fn quoted(l: &[u8]) -> Option<&[u8]> {
    let open = l.iter().position(|b| *b == b'"')?;
    let body = &l[open + 1..];
    let (mut i, mut last) = (0, None);
    while i < body.len() {
        match body[i] {
            b'\\' => i += 1,
            b'"' => last = Some(i),
            _ => {}
        }
        i += 1;
    }
    last.map(|e| &body[..e])
}

/// `key:N "value"` or `key "value"`; `None` when the line is not that shape.
fn entry(l: &[u8]) -> Option<Entry<'_>> {
    let key_len = l.iter().position(|b| *b == b':' || is_space(*b) || *b == b'"')?;
    let key = &l[..key_len];
    let mut rest = &l[key_len..];
    let mut ordinal = None;
    if let Some(r) = rest.strip_prefix(b":") {
        let digits = r.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits > 0 {
            ordinal = Some(std::str::from_utf8(&r[..digits]).ok()?.parse::<u16>().ok()?);
        }
        rest = &r[digits..];
    }
    let rest = trim_start(rest);
    if key.is_empty() || !rest.starts_with(b"\"") {
        return None;
    }
    Some(Entry { key, ordinal, value: quoted(rest)? })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    enc: Encoding,
    languages: &'a [Box<str>],
    lang: Lang,
    facts: Vec<RawFact>,
    diags: Diags,
}

impl Cursor<'_> {
    fn text(&self, b: &[u8]) -> Box<str> {
        match self.enc {
            Encoding::Utf8 => String::from_utf8_lossy(b).into(),
            Encoding::Windows1252 => crate::text::decode_1252(b).into(),
        }
    }

    fn header(&mut self, name: &[u8], at: Span) {
        let name = self.text(name);
        let found = self.languages.iter().position(|l| **l == *name);
        self.lang = match found.and_then(|i| u8::try_from(i).ok()) {
            Some(i) => Lang::Known(LangId(i)),
            None => {
                let detail = format!("language `{name}` is not in the profile; entries skipped");
                self.diags.push(codes::UNKNOWN_LANG, at, detail, true);
                Lang::Unlisted
            }
        };
    }

    fn entry(&mut self, e: &Entry<'_>, at: Span) {
        let lang = match self.lang {
            Lang::Known(l) => l,
            Lang::Unlisted => return,
            Lang::None => {
                let detail = "entry before any `l_<lang>:` header".to_owned();
                return self.diags.push(codes::NO_HEADER, at, detail, true);
            }
        };
        if e.key.len() > KEY_MAX {
            let detail = format!("key of {} bytes not extracted", e.key.len());
            return self.diags.push(codes::KEY_TOO_LONG, at, detail, true);
        }
        let key = self.text(e.key);
        if key.contains('\u{FFFD}') {
            let detail = "key holds a byte that does not decode".to_owned();
            return self.diags.push(codes::MALFORMED, at, detail, true);
        }
        let payload = FactPayload::LocKey {
            lang,
            key,
            version_ordinal: e.ordinal,
            value_hash: hash16(e.value),
        };
        self.facts.push(RawFact { span: at, payload });
    }

    fn line(&mut self, start: usize, raw: &[u8], line_no: u32) {
        let l = trim_start(raw);
        if l.is_empty() || l[0] == b'#' {
            return;
        }
        let offset = start + (raw.len() - l.len());
        let len = l.iter().rposition(|b| !is_space(*b)).map_or(0, |p| p + 1);
        let at = Span {
            offset: u32::try_from(offset).unwrap_or(u32::MAX),
            len: u32::try_from(len).unwrap_or(u32::MAX),
            line: line_no,
        };
        if let Some(name) = header(l) {
            self.header(name, at);
        } else if let Some(e) = entry(l) {
            self.entry(&e, at);
        } else {
            let detail = "not a header, an entry, or a comment".to_owned();
            self.diags.push(codes::MALFORMED, at, detail, true);
        }
    }
}

impl FileParser for LocYmlParser {
    fn id(&self) -> &'static str {
        "pdx_locyml"
    }

    fn version(&self) -> u32 {
        1
    }

    fn accepts(&self, kind: FileKind) -> bool {
        kind == FileKind::Localisation
    }

    fn parse(&self, input: &ParseInput<'_>) -> ParseOutput {
        let (start, enc) = detect(input.bytes);
        let mut cx = Cursor {
            bytes: input.bytes,
            enc,
            languages: &input.class.languages,
            lang: Lang::None,
            facts: Vec::new(),
            diags: Diags::default(),
        };
        if enc == Encoding::Windows1252 {
            let detail = "not UTF-8; decoded as Windows-1252".to_owned();
            cx.diags.push(codes::ENCODING, Span::default(), detail, false);
        }
        let mut at = start;
        for (n, raw) in cx.bytes[start..].split(|b| *b == b'\n').enumerate() {
            if n % POLL_LINES == 0 && (input.cancelled)() {
                // Cancelled: whatever was not read is unknown.
                cx.diags.partial = true;
                break;
            }
            cx.line(at, raw, u32::try_from(n + 1).unwrap_or(u32::MAX));
            at += raw.len() + 1;
        }
        ParseOutput::from_parts(cx.facts, cx.diags)
    }
}
