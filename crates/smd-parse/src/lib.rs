//! `smd-parse` — lexers, the parser registry, and fact emission.
//!
//! **Owns:** lexers for PDX script and localisation, the [`FileParser`] registry,
//! dispatch by [`FileKind`](smd_core::FileKind), fact emission, the parse-cache key.
//!
//! **Must never:** know about traits, civics, or any game concept; store anything.
//!
//! # Shape
//!
//! [`parse_file`] takes raw bytes and the path's [`DirClass`](smd_profiles::DirClass) and
//! returns a [`ParseOutput`]: string-based [`RawFact`](smd_core::RawFact)s, file-local
//! [`RawDiagnostic`](smd_core::RawDiagnostic)s, and a `ParseCompleteness`. Nothing here is
//! interned; `smd-index` interns on the way in, so the output is `Send` and cacheable.
//!
//! The script parser ([`script::parse`]) is event-driven and iterative with a depth cap
//! ([`DEPTH_MAX`]); every recovery is a diagnostic and makes the file `Partial`. Which key
//! is a definition is the class's `extractor` and `level` — profile data:
//!
//! | `extractor` | Emits |
//! | --- | --- |
//! | `depth_key` | `Definition` per key at `level`, plus `AdditiveSplit` where the class names `additive_children` |
//! | `define` | `DefineKey { group, key }` per key at `level` (the `depth_key` walk) |
//! | `pdx_event` | `Namespace`, `EventId` |
//! | `none` | nothing (syntax diagnostics only) |
//!
//! [`LocYmlParser`] (`pdx_locyml`) handles `FileKind::Localisation` and emits `LocKey`.
//! Adding a syntax is a module implementing [`FileParser`] and one entry in [`PARSERS`].

#![forbid(unsafe_code)]

mod cache;
mod clausewitz;
pub mod extract;
pub mod lexer;
pub mod locyml;
mod registry;
pub mod script;
pub mod text;

pub use cache::{ParseCache, ParseCacheKey};
pub use clausewitz::ClausewitzScriptParser;
pub use locyml::LocYmlParser;
pub use registry::{
    FILE_CAP, FileParser, PARSERS, ParseInput, ParseOutput, parse_file, parser_for,
};
pub use script::{DEPTH_MAX, ScriptSink};

#[cfg(test)]
mod tests;
