//! The effective extraction classification of one path.

use crate::files::FileKind;

/// Everything besides the bytes that decides which facts a parse produces. Part of the
/// parse-cache key (doc 06).
///
/// Identical bytes under `common/traits/` and `common/civics/` are two cache entries: the
/// cached `Definition` carries a domain, and the domain comes from the path. The same
/// holds for `additive_children`: identical on_action bytes under two profiles with
/// different child sets split differently.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ExtractionClass {
    /// Dispatch class.
    pub kind: FileKind,
    /// `FileParser::id()`.
    pub parser: &'static str,
    /// `"depth_key"`, `"event"`, `"none"`, or a named extractor.
    pub extractor: &'static str,
    /// Extraction depth.
    pub level: u8,
    /// Domain *name*, not `DomainId`.
    pub domain: Option<Box<str>>,
    /// The profile's `additive_children` for this path. Non-empty only where the
    /// extractor emits `AdditiveSplit` facts. Order given is irrelevant:
    /// [`key_bytes`](ExtractionClass::key_bytes) sorts.
    pub additive_children: Box<[Box<str>]>,
}

fn frame(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

impl ExtractionClass {
    /// Canonical, length-prefixed bytes for hashing into a cache key.
    ///
    /// Every variable-length field is framed, so `("ab", "c")` and `("a", "bc")` differ,
    /// and `additive_children` is sorted, so the order a profile lists them in does not
    /// change the key.
    #[must_use]
    pub fn key_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.push(self.kind as u8); // explicit discriminants, see `FileKind`
        frame(&mut out, self.parser.as_bytes());
        frame(&mut out, self.extractor.as_bytes());
        out.push(self.level);
        match &self.domain {
            None => out.push(0),
            Some(d) => {
                out.push(1);
                frame(&mut out, d.as_bytes());
            }
        }
        let mut children: Vec<&str> = self.additive_children.iter().map(|c| &**c).collect();
        children.sort_unstable();
        children.dedup();
        out.extend_from_slice(&(children.len() as u64).to_le_bytes());
        for c in children {
            frame(&mut out, c.as_bytes());
        }
        out
    }
}
