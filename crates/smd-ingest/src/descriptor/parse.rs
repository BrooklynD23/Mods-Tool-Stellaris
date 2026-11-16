//! The descriptor grammar: `key = value` statements, with `{ ... }` lists.
//!
//! An explicit frame stack rather than recursion, so depth is a counter checked against
//! [`DEPTH_MAX`] and not the call stack.

use smallvec::SmallVec;
use smd_core::{Descriptor, VersionComponent, VersionPattern};

use super::lexer::{LexError, Lexer, Tok};
use super::{DEPTH_MAX, LIST_MAX, VALUE_MAX};

/// Why parsing stopped early.
#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum ParseError {
    /// A cap tripped: the descriptor is rejected whole.
    Cap(String),
    /// The text is malformed; fields read before the fault are kept.
    Syntax(String),
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ListKind {
    Tags,
    Dependencies,
    ReplacePath,
}

enum Frame {
    List(ListKind),
    Skip,
}

struct Parser {
    desc: Descriptor,
    frames: Vec<Frame>,
}

/// Parse `bytes`. The descriptor holds whatever was read before `error`, if any.
pub(super) fn parse(bytes: &[u8]) -> (Descriptor, Option<ParseError>) {
    let mut p = Parser { desc: Descriptor::default(), frames: Vec::new() };
    let error = p.run(&mut Lexer::new(bytes)).err();
    (p.desc, error)
}

fn lex(e: &LexError) -> ParseError {
    match e {
        LexError::ValueTooLong => ParseError::Cap(format!("a value exceeds {VALUE_MAX} bytes")),
        LexError::UnterminatedString => ParseError::Syntax("unterminated string".into()),
    }
}

impl Parser {
    fn run(&mut self, lx: &mut Lexer<'_>) -> Result<(), ParseError> {
        while let Some(tok) = lx.next_tok().map_err(|e| lex(&e))? {
            match tok {
                Tok::Str(s) => self.statement(lx, s)?,
                Tok::Open => self.push(Frame::Skip)?,
                Tok::Close => {
                    if self.frames.pop().is_none() {
                        return Err(ParseError::Syntax("unbalanced `}`".into()));
                    }
                }
                Tok::Eq => return Err(ParseError::Syntax("`=` without a key".into())),
            }
        }
        if self.frames.is_empty() { Ok(()) } else { Err(ParseError::Syntax("unclosed `{`".into())) }
    }

    fn push(&mut self, f: Frame) -> Result<(), ParseError> {
        if self.frames.len() >= DEPTH_MAX {
            return Err(ParseError::Cap(format!("nesting deeper than {DEPTH_MAX}")));
        }
        self.frames.push(f);
        Ok(())
    }

    fn statement(&mut self, lx: &mut Lexer<'_>, key: Box<str>) -> Result<(), ParseError> {
        let has_eq = matches!(lx.peek().map_err(|e| lex(&e))?, Some(Tok::Eq));
        if !has_eq {
            return match self.frames.last() {
                Some(Frame::List(kind)) => self.item(*kind, key),
                _ => Ok(()),
            };
        }
        lx.next_tok().map_err(|e| lex(&e))?;
        let at_root = self.frames.is_empty();
        match lx.next_tok().map_err(|e| lex(&e))? {
            Some(Tok::Str(v)) if at_root => self.assign(&key, v),
            Some(Tok::Str(_)) => Ok(()),
            Some(Tok::Open) => {
                let kind = if at_root { list_kind(&key) } else { None };
                self.push(kind.map_or(Frame::Skip, Frame::List))
            }
            _ => Err(ParseError::Syntax(format!("`{key}` has no value"))),
        }
    }

    fn item(&mut self, kind: ListKind, v: Box<str>) -> Result<(), ParseError> {
        let list = match kind {
            ListKind::Tags => &mut self.desc.tags,
            ListKind::Dependencies => &mut self.desc.dependencies,
            ListKind::ReplacePath => &mut self.desc.replace_paths,
        };
        if list.len() >= LIST_MAX {
            return Err(ParseError::Cap(format!("more than {LIST_MAX} list entries")));
        }
        list.push(v);
        Ok(())
    }

    fn assign(&mut self, key: &str, v: Box<str>) -> Result<(), ParseError> {
        match key {
            "name" => self.desc.name = v,
            "path" => self.desc.path = Some(v.as_ref().into()),
            "archive" => self.desc.archive = Some(v.as_ref().into()),
            "remote_file_id" => self.desc.remote_file_id = Some(v),
            "version" => self.desc.version = Some(v),
            "supported_version" => self.desc.supported_version = pattern(&v),
            "replace_path" => return self.item(ListKind::ReplacePath, v),
            "tags" => return self.item(ListKind::Tags, v),
            "dependencies" => return self.item(ListKind::Dependencies, v),
            _ => {}
        }
        Ok(())
    }
}

fn list_kind(key: &str) -> Option<ListKind> {
    match key {
        "tags" => Some(ListKind::Tags),
        "dependencies" => Some(ListKind::Dependencies),
        "replace_path" => Some(ListKind::ReplacePath),
        _ => None,
    }
}

/// `3.13.*` -> `[Exact(3), Exact(13), Wildcard]`; anything unparseable is `None`.
fn pattern(raw: &str) -> Option<VersionPattern> {
    let mut parts: SmallVec<[VersionComponent; 4]> = SmallVec::new();
    for piece in raw.trim().split('.') {
        if piece == "*" {
            parts.push(VersionComponent::Wildcard);
            break;
        }
        parts.push(VersionComponent::Exact(piece.parse().ok()?));
    }
    Some(VersionPattern(parts))
}
