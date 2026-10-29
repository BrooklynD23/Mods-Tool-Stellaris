//! The bounded KeyValues lexer behind the descriptor reader.
//!
//! Hand-written, iterative, and bounded: no recursion, and every string is checked
//! against the value cap while it is being built, so an adversarial file cannot make the
//! lexer allocate past the cap before it notices. (Doc 05, "Descriptor, metadata, and
//! text limits"; gate H24.)

use super::VALUE_MAX;

/// One token of the dialect.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum Tok {
    /// A quoted string or a bare word, decoded.
    Str(Box<str>),
    /// `=`.
    Eq,
    /// `{`.
    Open,
    /// `}`.
    Close,
}

/// Why lexing stopped.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum LexError {
    /// A single string grew past [`VALUE_MAX`].
    ValueTooLong,
    /// A quoted string ran to the end of the input.
    UnterminatedString,
}

/// A cursor over the input, with one token of lookahead.
pub(super) struct Lexer<'a> {
    bytes: &'a [u8],
    pos: usize,
    peeked: Option<Option<Tok>>,
}

fn is_delimiter(b: u8) -> bool {
    b.is_ascii_whitespace() || matches!(b, b'=' | b'{' | b'}' | b'"' | b'#')
}

impl<'a> Lexer<'a> {
    /// A lexer over `bytes`, past a UTF-8 byte-order mark when there is one.
    pub(super) fn new(bytes: &'a [u8]) -> Lexer<'a> {
        let pos = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
        Lexer { bytes, pos, peeked: None }
    }

    /// The next token without consuming it.
    pub(super) fn peek(&mut self) -> Result<Option<&Tok>, LexError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.scan()?);
        }
        Ok(self.peeked.as_ref().and_then(Option::as_ref))
    }

    /// Consume and return the next token; `None` at the end of the input.
    pub(super) fn next_tok(&mut self) -> Result<Option<Tok>, LexError> {
        match self.peeked.take() {
            Some(t) => Ok(t),
            None => self.scan(),
        }
    }

    fn skip_trivia(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b.is_ascii_whitespace() {
                self.pos += 1;
            } else if b == b'#' {
                while self.bytes.get(self.pos).is_some_and(|&c| c != b'\n') {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn scan(&mut self) -> Result<Option<Tok>, LexError> {
        self.skip_trivia();
        let Some(&b) = self.bytes.get(self.pos) else { return Ok(None) };
        let simple = match b {
            b'=' => Some(Tok::Eq),
            b'{' => Some(Tok::Open),
            b'}' => Some(Tok::Close),
            _ => None,
        };
        if simple.is_some() {
            self.pos += 1;
            return Ok(simple);
        }
        if b == b'"' {
            self.pos += 1;
            return self.quoted().map(Some);
        }
        self.bare().map(Some)
    }

    fn bare(&mut self) -> Result<Tok, LexError> {
        let start = self.pos;
        while self.bytes.get(self.pos).is_some_and(|&c| !is_delimiter(c)) {
            self.pos += 1;
            if self.pos - start > VALUE_MAX {
                return Err(LexError::ValueTooLong);
            }
        }
        Ok(Tok::Str(String::from_utf8_lossy(&self.bytes[start..self.pos]).into()))
    }

    /// A quoted string. `\"` is a quote and `\\` keeps both backslashes (so the second
    /// cannot escape a closing quote); any other backslash is literal, which keeps
    /// Windows paths intact. Everything else, control characters included, is verbatim.
    fn quoted(&mut self) -> Result<Tok, LexError> {
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let Some(&b) = self.bytes.get(self.pos) else {
                return Err(LexError::UnterminatedString);
            };
            self.pos += 1;
            match (b, self.bytes.get(self.pos)) {
                (b'"', _) => break,
                (b'\\', Some(b'"')) => {
                    buf.push(b'"');
                    self.pos += 1;
                }
                (b'\\', Some(b'\\')) => {
                    buf.extend_from_slice(b"\\\\");
                    self.pos += 1;
                }
                _ => buf.push(b),
            }
            if buf.len() > VALUE_MAX {
                return Err(LexError::ValueTooLong);
            }
        }
        Ok(Tok::Str(String::from_utf8_lossy(&buf).into()))
    }
}
