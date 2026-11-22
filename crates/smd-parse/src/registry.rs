//! The parser registry (doc 06 § Parser registry), dispatched by [`FileKind`].
//!
//! Explicit and sorted rather than `inventory`-collected, like the rule registry: a new
//! syntax is one module implementing [`FileParser`] plus one line in [`PARSERS`].

use smd_core::{FileKind, ParseCompleteness, RawDiagnostic, RawFact, UnparsedReason};
use smd_profiles::DirClass;

use crate::script::Diags;

/// Largest file that is parsed (doc 05 § Unbounded trees, doc 06): 32 MB.
pub const FILE_CAP: u64 = 32 * 1024 * 1024;

/// One file to parse: its raw bytes and its path's classification.
pub struct ParseInput<'a> {
    /// The raw file, BOM included.
    pub bytes: &'a [u8],
    /// The profile's classification of the path.
    pub class: &'a DirClass,
    /// Polled in the parser's outer loop.
    pub cancelled: &'a dyn Fn() -> bool,
}

/// Everything a parse produced: the cacheable value (doc 06 `CachedParse`). Facts and
/// diagnostics are file-local and string-based, so this is `Send` and may cross a thread
/// or a cache boundary; it is sited and interned only on the way into the index.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseOutput {
    /// Facts in input order.
    pub facts: Vec<RawFact>,
    /// Recovery and encoding diagnostics.
    pub diags: Vec<RawDiagnostic>,
    /// `Complete`, `Partial`, or `Unparsed`; never `NotApplicable`.
    pub completeness: ParseCompleteness,
}

impl ParseOutput {
    /// No facts, because the file was not parsed.
    #[must_use]
    pub fn unparsed(why: UnparsedReason) -> ParseOutput {
        ParseOutput {
            facts: Vec::new(),
            diags: Vec::new(),
            completeness: ParseCompleteness::Unparsed(why),
        }
    }

    pub(crate) fn from_parts(facts: Vec<RawFact>, diags: Diags) -> ParseOutput {
        let (diags, partial) = diags.finish();
        let completeness =
            if partial { ParseCompleteness::Partial } else { ParseCompleteness::Complete };
        ParseOutput { facts, diags, completeness }
    }
}

/// A syntax: turns bytes into facts. Holds no state beyond one call.
pub trait FileParser: Sync {
    /// Stable id, part of the parse-cache key.
    fn id(&self) -> &'static str;
    /// Bump to invalidate exactly this parser's cache entries.
    fn version(&self) -> u32;
    /// Whether this parser handles `kind`.
    fn accepts(&self, kind: FileKind) -> bool;
    /// Parse one file. Never panics on any byte sequence by design; recovery is a
    /// diagnostic and `Partial`.
    fn parse(&self, input: &ParseInput<'_>) -> ParseOutput;
}

/// Every registered parser.
pub static PARSERS: &[&dyn FileParser] = &[&crate::ClausewitzScriptParser, &crate::LocYmlParser];

/// The parser for a kind, if any accepts it.
#[must_use]
pub fn parser_for(kind: FileKind) -> Option<&'static dyn FileParser> {
    PARSERS.iter().copied().find(|p| p.accepts(kind))
}

/// Parse with whichever parser accepts the class's kind. Over [`FILE_CAP`] is
/// `Unparsed(Oversize)`; no parser is `Unparsed(NoParser)`.
#[must_use]
pub fn parse_file(input: &ParseInput<'_>) -> ParseOutput {
    if input.bytes.len() as u64 > FILE_CAP {
        return ParseOutput::unparsed(UnparsedReason::Oversize);
    }
    match parser_for(input.class.kind) {
        Some(p) => p.parse(input),
        None => ParseOutput::unparsed(UnparsedReason::NoParser),
    }
}
