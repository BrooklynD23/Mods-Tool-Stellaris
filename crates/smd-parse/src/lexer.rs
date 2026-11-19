//! The Clausewitz lexer: a byte cursor over a `&[u8]` that never allocates.
//!
//! Tokens are spans into the raw file. Comments and whitespace are skipped; newlines are
//! not significant except for the advisory line number.

/// A comparison or assignment operator.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Op {
    /// `=`
    Eq,
    /// `==`
    EqEq,
    /// `>`
    Gt,
    /// `<`
    Lt,
    /// `>=`
    Ge,
    /// `<=`
    Le,
    /// `!=`
    Ne,
    /// `?=`
    QEq,
}

/// What a token is.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TokKind {
    /// An unquoted run: a key, a number, `yes`, `@var`, `@[ maths ]`, `$PARAM$`, `[[PARAM]`.
    Word,
    /// `"…"`, quotes included in the span.
    Quoted,
    /// An operator.
    Op(Op),
    /// `{`
    Open,
    /// `}`
    Close,
    /// End of input.
    Eof,
}

/// One token: a raw-file byte range and the 1-based line it starts on.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Token {
    /// What it is.
    pub kind: TokKind,
    /// First byte.
    pub start: u32,
    /// One past the last byte.
    pub end: u32,
    /// 1-based line, advisory.
    pub line: u32,
}

/// A cursor with one token of lookahead.
pub struct Lexer<'a> {
    b: &'a [u8],
    pos: usize,
    line: u32,
    peeked: Option<Token>,
    /// A quoted string ran to the end of input.
    pub unterminated: bool,
}

fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c)
}

fn ends_word(c: u8) -> bool {
    is_ws(c) || matches!(c, b'{' | b'}' | b'=' | b'<' | b'>' | b'#' | b'"')
}

fn off(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl<'a> Lexer<'a> {
    /// Lex `b` from byte `start` (past a BOM, or the start of a body).
    #[must_use]
    pub fn new(b: &'a [u8], start: usize) -> Lexer<'a> {
        Lexer { b, pos: start, line: 1, peeked: None, unterminated: false }
    }

    /// Lex from `start`, reporting lines as if `line` were the current one.
    #[must_use]
    pub fn at_line(b: &'a [u8], start: usize, line: u32) -> Lexer<'a> {
        Lexer { line, ..Lexer::new(b, start) }
    }

    /// The next token without consuming it.
    pub fn peek(&mut self) -> Token {
        match self.peeked {
            Some(t) => t,
            None => {
                let t = self.lex();
                self.peeked = Some(t);
                t
            }
        }
    }

    /// Consume the next token.
    pub fn next_token(&mut self) -> Token {
        self.peeked.take().unwrap_or_else(|| self.lex())
    }

    fn at(&self, i: usize) -> Option<u8> {
        self.b.get(i).copied()
    }

    fn advance_to(&mut self, end: usize) {
        let end = end.min(self.b.len());
        self.line += off(self.b[self.pos..end].iter().filter(|&&c| c == b'\n').count());
        self.pos = end;
    }

    fn skip_trivia(&mut self) {
        while let Some(c) = self.at(self.pos) {
            if c == b'\n' {
                self.line += 1;
                self.pos += 1;
            } else if is_ws(c) {
                self.pos += 1;
            } else if c == b'#' {
                let eol = self.b[self.pos..].iter().position(|&c| c == b'\n');
                self.pos = eol.map_or(self.b.len(), |n| self.pos + n);
            } else {
                return;
            }
        }
    }

    fn lex(&mut self) -> Token {
        self.skip_trivia();
        let (start, line) = (self.pos, self.line);
        let Some(c) = self.at(start) else {
            return Token { kind: TokKind::Eof, start: off(start), end: off(start), line };
        };
        let next = self.at(start + 1);
        let (kind, end) = match (c, next) {
            (b'{', _) => (TokKind::Open, start + 1),
            (b'}', _) => (TokKind::Close, start + 1),
            (b'"', _) => (TokKind::Quoted, self.quoted_end(start)),
            (b'=', Some(b'=')) => (TokKind::Op(Op::EqEq), start + 2),
            (b'=', _) => (TokKind::Op(Op::Eq), start + 1),
            (b'>', Some(b'=')) => (TokKind::Op(Op::Ge), start + 2),
            (b'>', _) => (TokKind::Op(Op::Gt), start + 1),
            (b'<', Some(b'=')) => (TokKind::Op(Op::Le), start + 2),
            (b'<', _) => (TokKind::Op(Op::Lt), start + 1),
            (b'!', Some(b'=')) => (TokKind::Op(Op::Ne), start + 2),
            (b'?', Some(b'=')) => (TokKind::Op(Op::QEq), start + 2),
            (b'@', Some(b'[')) => (TokKind::Word, self.maths_end(start)),
            _ => (TokKind::Word, self.word_end(start)),
        };
        self.advance_to(end);
        Token { kind, start: off(start), end: off(end), line }
    }

    /// One past the closing quote; a backslash escapes the next byte.
    fn quoted_end(&mut self, start: usize) -> usize {
        let mut i = start + 1;
        while let Some(c) = self.at(i) {
            match c {
                b'\\' => i += 2,
                b'"' => return i + 1,
                _ => i += 1,
            }
        }
        self.unterminated = true;
        self.b.len()
    }

    /// `@[ … ]`: one word up to the first `]`.
    fn maths_end(&self, start: usize) -> usize {
        self.b[start..].iter().position(|&c| c == b']').map_or(self.b.len(), |n| start + n + 1)
    }

    fn word_end(&self, start: usize) -> usize {
        let mut i = start;
        while let Some(c) = self.at(i) {
            let op_start = matches!(c, b'!' | b'?') && self.at(i + 1) == Some(b'=');
            if ends_word(c) || (op_start && i > start) {
                break;
            }
            i += 1;
        }
        i.max(start + 1)
    }
}
