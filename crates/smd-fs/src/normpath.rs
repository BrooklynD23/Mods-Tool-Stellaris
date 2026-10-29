//! Path normalisation: the identity key of a file (doc 03, "Files").
//!
//! The key is identity only. The OS path used to open a file is the raw directory-entry
//! name carried beside it, never reconstructed from the key.

use unicode_normalization::UnicodeNormalization;

/// A normalised, mod-root-relative path: Unicode NFC, locale-independent lowercase, `/`
/// separators, no leading slash.
///
/// Ordering is by UTF-8 bytes, which is the order `PathId`s are assigned in.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct NormPath(Box<str>);

/// Why a path could not be normalised.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum NormError {
    /// The path (after stripping leading separators) has no components.
    Empty,
    /// A component is empty, as in `a//b` or a trailing separator.
    EmptyComponent,
    /// A component is `..`, which could leave the root.
    ParentComponent,
    /// A component is `.`, which is not a canonical spelling.
    CurrentComponent,
}

impl std::fmt::Display for NormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            NormError::Empty => "empty path",
            NormError::EmptyComponent => "empty path component",
            NormError::ParentComponent => "`..` path component",
            NormError::CurrentComponent => "`.` path component",
        })
    }
}

impl std::error::Error for NormError {}

impl NormPath {
    /// Normalise `input` into an identity key.
    ///
    /// `\` becomes `/`, leading separators are dropped, each component is NFC-normalised
    /// and lowercased with `str::to_lowercase` (Unicode default case mapping, no Turkic
    /// tailoring). `..`, `.` and empty components are rejected.
    ///
    /// # Errors
    /// Returns a [`NormError`] naming the offending shape.
    pub fn new(input: &str) -> Result<NormPath, NormError> {
        let unified = input.replace('\\', "/");
        let trimmed = unified.trim_start_matches('/');
        if trimmed.is_empty() {
            return Err(NormError::Empty);
        }
        let mut out = String::with_capacity(trimmed.len());
        for (i, part) in trimmed.split('/').enumerate() {
            match part {
                "" => return Err(NormError::EmptyComponent),
                ".." => return Err(NormError::ParentComponent),
                "." => return Err(NormError::CurrentComponent),
                _ => {}
            }
            if i > 0 {
                out.push('/');
            }
            out.push_str(&fold(part));
        }
        Ok(NormPath(out.into_boxed_str()))
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The key as UTF-8 bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl std::fmt::Display for NormPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// NFC, then lowercase, then NFC again: lowercasing can emit a sequence that is not
/// itself in NFC.
fn fold(part: &str) -> String {
    let lowered = part.nfc().collect::<String>().to_lowercase();
    lowered.nfc().collect()
}

/// A name as shipped: NFC only, case preserved.
#[must_use]
pub fn shipped_name(name: &str) -> Box<str> {
    name.nfc().collect::<String>().into_boxed_str()
}

#[cfg(test)]
mod tests;
