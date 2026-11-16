//! The descriptor reader: `descriptor.mod` and the launcher's `*.mod` (doc 05).
//!
//! Both are one KeyValues dialect. The reader is hand-written and bounded, because a
//! descriptor is the first thing an adversarial Workshop upload reaches (gate H24).

use smd_core::{Descriptor, DescriptorReader, DiagSeverity, Diagnostic, Site};

mod lexer;
mod parse;

use parse::ParseError;

/// Largest descriptor read, in bytes.
pub const FILE_MAX: usize = 64 * 1024;
/// Longest single value, in bytes.
pub const VALUE_MAX: usize = 4 * 1024;
/// Most `tags`, `dependencies`, or `replace_path` entries.
pub const LIST_MAX: usize = 256;
/// Deepest `{ }` nesting.
pub const DEPTH_MAX: usize = 8;

/// The registry key of the KeyValues reader.
pub const READER_ID: &str = "keyvalues";

/// The shipped [`DescriptorReader`].
#[derive(Copy, Clone, Debug, Default)]
pub struct KeyValuesReader;

fn diag(site: Site, code: &'static str, severity: DiagSeverity, detail: String) -> Diagnostic {
    Diagnostic { site, code, severity, detail: detail.into_boxed_str() }
}

impl DescriptorReader for KeyValuesReader {
    fn id(&self) -> &'static str {
        READER_ID
    }

    fn read(&self, bytes: &[u8], site: Site) -> (Descriptor, Vec<Diagnostic>) {
        if bytes.len() > FILE_MAX {
            let why = format!("descriptor is {} bytes, over the {FILE_MAX}-byte cap", bytes.len());
            let d = diag(site, crate::codes::DESCRIPTOR_INVALID, DiagSeverity::Error, why);
            return (Descriptor::default(), vec![d]);
        }
        let lossy = std::str::from_utf8(bytes).is_err();
        let (desc, mut diags) = self.parsed(bytes, site);
        if lossy && !diags.iter().any(|d| d.code == crate::codes::DESCRIPTOR_INVALID) {
            let why = "descriptor is not valid UTF-8; invalid bytes were decoded as U+FFFD, \
                       so values are not verbatim";
            diags.push(diag(site, crate::codes::DESCRIPTOR_SYNTAX, DiagSeverity::Warn, why.into()));
        }
        (desc, diags)
    }
}

impl KeyValuesReader {
    fn parsed(&self, bytes: &[u8], site: Site) -> (Descriptor, Vec<Diagnostic>) {
        match parse::parse(bytes) {
            (desc, None) => (desc, Vec::new()),
            (_, Some(ParseError::Cap(why))) => {
                let d = diag(site, crate::codes::DESCRIPTOR_INVALID, DiagSeverity::Error, why);
                (Descriptor::default(), vec![d])
            }
            (desc, Some(ParseError::Syntax(why))) => {
                (desc, vec![diag(site, crate::codes::DESCRIPTOR_SYNTAX, DiagSeverity::Warn, why)])
            }
        }
    }
}

#[cfg(test)]
mod tests;
