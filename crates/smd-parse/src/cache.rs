//! The layer-2 parse-cache key and an in-memory cache seam (doc 06 § Parse caching, H10).
//!
//! The key names every input of a parse: `content_hash ‖ parser_id ‖ parser_version ‖
//! CONTRACT_VERSION ‖ profile_hash ‖ ExtractionClass`. Identical bytes under two
//! directories with different classifications are two entries; identical bytes vendored
//! by five mods under one classification are one.
//!
//! The value is the whole [`ParseOutput`] — facts, diagnostics, completeness — so a warm
//! hit replays the cold parse's recoveries (H11). `Unparsed` results are never cached:
//! their reason belongs to the scan, not to the bytes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use smd_core::{CONTRACT_VERSION, ExtractionClass, ParseCompleteness};

use crate::registry::{FileParser, ParseOutput};

/// The complete key. Field order is the canonical byte order of [`ParseCacheKey::to_bytes`].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct ParseCacheKey {
    /// blake3 of the raw file bytes, truncated.
    pub content_hash: [u8; 16],
    /// `FileParser::id()`.
    pub parser_id: &'static str,
    /// `FileParser::version()`.
    pub parser_version: u32,
    /// [`CONTRACT_VERSION`] at the time of the parse.
    pub contract_version: u32,
    /// blake3 of the profile text, truncated.
    pub profile_hash: [u8; 16],
    /// [`ExtractionClass::key_bytes`] of the path.
    pub class: Box<[u8]>,
}

impl ParseCacheKey {
    /// The key for parsing bytes hashing to `content_hash` with `parser`, under a profile
    /// hashing to `profile_hash`, at a path classified as `class`.
    #[must_use]
    pub fn new(
        content_hash: [u8; 16],
        parser: &dyn FileParser,
        profile_hash: [u8; 16],
        class: &ExtractionClass,
    ) -> ParseCacheKey {
        ParseCacheKey {
            content_hash,
            parser_id: parser.id(),
            parser_version: parser.version(),
            contract_version: CONTRACT_VERSION,
            profile_hash,
            class: class.key_bytes().into_boxed_slice(),
        }
    }

    /// Canonical, length-framed bytes: the on-disk key, when there is a disk.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.class.len());
        out.extend_from_slice(&self.content_hash);
        out.extend_from_slice(&(self.parser_id.len() as u64).to_le_bytes());
        out.extend_from_slice(self.parser_id.as_bytes());
        out.extend_from_slice(&self.parser_version.to_le_bytes());
        out.extend_from_slice(&self.contract_version.to_le_bytes());
        out.extend_from_slice(&self.profile_hash);
        out.extend_from_slice(&(self.class.len() as u64).to_le_bytes());
        out.extend_from_slice(&self.class);
        out
    }
}

/// In-memory, scan-lifetime cache. `Sync`, so a parallel parse stage can share it.
///
/// ponytail: one global mutex and no eviction; the SQLite store with LRU (doc 06 § Store
/// and eviction) replaces it when warm scans across runs are wanted.
#[derive(Debug, Default)]
pub struct ParseCache {
    map: Mutex<HashMap<ParseCacheKey, Arc<ParseOutput>>>,
}

impl ParseCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> ParseCache {
        ParseCache::default()
    }

    /// The cached parse for `key`.
    #[must_use]
    pub fn get(&self, key: &ParseCacheKey) -> Option<Arc<ParseOutput>> {
        self.map.lock().unwrap_or_else(PoisonError::into_inner).get(key).cloned()
    }

    /// Store a parse; an `Unparsed` result is dropped, never cached.
    pub fn insert(&self, key: ParseCacheKey, value: Arc<ParseOutput>) {
        if matches!(value.completeness, ParseCompleteness::Unparsed(_)) {
            return;
        }
        self.map.lock().unwrap_or_else(PoisonError::into_inner).insert(key, value);
    }

    /// How many entries are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
