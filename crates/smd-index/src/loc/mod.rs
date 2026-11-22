//! The localisation layer (doc 07 § `LocTable`): every `LocKey` fact, sited and interned,
//! grouped by `(LangId, key)`, and resolved through Stages C and D with the class `tier`
//! (`localisation/replace/`-style entries, H3) compared first.

mod resolve;

pub use resolve::LocVerdict;

use std::collections::BTreeMap;

use smallvec::SmallVec;
use smd_core::{FactPayload, LangId, PathId, Site, SymbolId};

use crate::defs::{BuildCtx, FactBatch};
use crate::interner::{Interner, InternerBuilder};

/// One entry of one key in one language.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct LocSite {
    /// Which mod, which file, which bytes.
    pub site: Site,
    /// blake3 over the value as written, truncated.
    pub value_hash: [u8; 16],
    /// The `:N` suffix; `None` when the entry declared none.
    pub version_ordinal: Option<u16>,
    /// The overlay slot of the file holding it.
    pub file_slot: u32,
    /// Position among the file's entries: the in-file ordinal of Stage D.
    pub ordinal: u32,
}

/// `(LangId, key)` → its entries, ascending by key; each list ascending by
/// `(source index, path, ordinal)`.
#[derive(Clone, Debug, Default)]
pub struct LocTable {
    map: BTreeMap<(LangId, SymbolId), SmallVec<[LocSite; 2]>>,
}

impl LocTable {
    /// The entries of `key` in `lang`.
    #[must_use]
    pub fn sites(&self, lang: LangId, key: SymbolId) -> &[LocSite] {
        self.map.get(&(lang, key)).map_or(&[], |v| v.as_slice())
    }

    /// Every `(lang, key)` and its entries, ascending.
    pub fn iter(&self) -> impl Iterator<Item = ((LangId, SymbolId), &[LocSite])> + '_ {
        self.map.iter().map(|(k, v)| (*k, v.as_slice()))
    }

    /// Keys with at least two entries on disk.
    pub fn multi_defined(&self) -> impl Iterator<Item = ((LangId, SymbolId), &[LocSite])> + '_ {
        self.iter().filter(|(_, v)| v.len() >= 2)
    }

    /// How many `(lang, key)` pairs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no entry was extracted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Put every localisation key into the symbol interner.
pub(crate) fn intern(sb: &mut InternerBuilder, batches: &[FactBatch]) {
    for f in batches.iter().flat_map(|b| &b.facts) {
        if let FactPayload::LocKey { key, .. } = &f.payload {
            sb.insert(key);
        }
    }
}

fn sym(symbols: &Interner, s: &str) -> SymbolId {
    SymbolId(symbols.find(s).unwrap_or(0))
}

/// Site and group every batch's `LocKey` facts. Facts of other kinds are other tables'.
pub(crate) fn build(batches: &[FactBatch], cx: &BuildCtx<'_>) -> LocTable {
    let mut map: BTreeMap<(LangId, SymbolId), SmallVec<[LocSite; 2]>> = BTreeMap::new();
    for b in batches {
        let Some(pid) = cx.paths.find(&b.path).map(PathId) else { continue };
        let Some(file_slot) = cx.overlay.slot_of(pid, b.source) else { continue };
        let mut ordinal = 0u32;
        for f in &b.facts {
            if let FactPayload::LocKey { lang, key, version_ordinal, value_hash } = &f.payload {
                let site = Site { source: b.source, path: pid, span: f.span };
                let entry = LocSite {
                    site,
                    value_hash: *value_hash,
                    version_ordinal: *version_ordinal,
                    file_slot,
                    ordinal,
                };
                map.entry((*lang, sym(cx.symbols, key))).or_default().push(entry);
                ordinal += 1;
            }
        }
    }
    for v in map.values_mut() {
        v.sort_by_key(|d: &LocSite| (d.site.source.index(), d.site.path, d.ordinal));
    }
    LocTable { map }
}
