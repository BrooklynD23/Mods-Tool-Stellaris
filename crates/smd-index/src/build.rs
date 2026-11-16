//! `IndexBuilder` — assembles the file layer from per-source walk records.

use std::sync::Arc;

use smallvec::SmallVec;
use smd_core::{
    Blob, Diagnostic, ParseCompleteness, PathId, PlaysetProvenance, SourceId, SourceTable, SymbolId,
};
use smd_fs::{NormPath, WalkRecord};

use crate::derived::{BitSet, Derived};
use crate::index::ScanIndex;
use crate::interner::InternerBuilder;
use crate::overlay::{Contribution, OverlayTable};
use crate::overlay_rules::OverlayRules;
use crate::replace::{ReplaceDecl, normalise_prefix};

/// One file as the index sees it: the part of a walk record Stage A reads.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FileRecord {
    /// Identity key.
    pub path: NormPath,
    /// As-shipped spelling; a sort key, never identity.
    pub raw_name: Box<str>,
    /// Content address, when established.
    pub blob: Option<Blob>,
    /// How completely it was read.
    pub completeness: ParseCompleteness,
    /// Another entry of the same source has the same identity key.
    pub ambiguous: bool,
}

impl From<&WalkRecord> for FileRecord {
    fn from(r: &WalkRecord) -> FileRecord {
        FileRecord {
            path: r.path.clone(),
            raw_name: r.raw_name.clone(),
            blob: r.blob,
            completeness: r.completeness,
            ambiguous: r.ambiguous,
        }
    }
}

/// Everything one source contributes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SourceInput {
    /// Which source.
    pub source: SourceId,
    /// Its files, in any order.
    pub records: Vec<FileRecord>,
    /// Its declared `replace_path` prefixes.
    pub replace_paths: Vec<String>,
}

/// Collects inputs, then freezes them into a [`ScanIndex`].
pub struct IndexBuilder {
    sources: SourceTable,
    provenance: PlaysetProvenance,
    rules: OverlayRules,
    inputs: Vec<SourceInput>,
    diags: Vec<Diagnostic>,
}

type Row<'a> = (u32, u32, &'a str, Option<Blob>, &'a FileRecord);

impl IndexBuilder {
    /// A builder over the source table and the profile-derived overlay rules.
    #[must_use]
    pub fn new(
        sources: SourceTable,
        provenance: PlaysetProvenance,
        rules: OverlayRules,
    ) -> IndexBuilder {
        IndexBuilder { sources, provenance, rules, inputs: Vec::new(), diags: Vec::new() }
    }

    /// Add one source's walk. Call order does not affect the result.
    pub fn add_source(&mut self, input: SourceInput) -> &mut Self {
        self.inputs.push(input);
        self
    }

    /// Carry pipeline diagnostics into the index.
    pub fn add_diagnostics(&mut self, diags: impl IntoIterator<Item = Diagnostic>) -> &mut Self {
        self.diags.extend(diags);
        self
    }

    /// Assign ids (ascending `NormPath` bytes), build the overlay, and run Stages A and B.
    #[must_use]
    pub fn build(self) -> ScanIndex {
        let (mut pb, mut sb) = (InternerBuilder::new(), InternerBuilder::new());
        for r in self.inputs.iter().flat_map(|i| &i.records) {
            pb.insert(r.path.as_str());
            sb.insert(&r.raw_name);
        }
        // `replace_path` prefixes are subjects too (`Subject::Directory`), so they need ids.
        for prefix in self.inputs.iter().flat_map(|i| &i.replace_paths) {
            let prefix = normalise_prefix(prefix);
            if !prefix.is_empty() {
                pb.insert(&prefix);
            }
        }
        let (paths, symbols) = (Arc::new(pb.freeze()), Arc::new(sb.freeze()));
        let mut rows: Vec<Row<'_>> = Vec::new();
        for i in &self.inputs {
            for r in &i.records {
                let pid = paths.find(r.path.as_str()).unwrap_or(0);
                rows.push((pid, i.source.index(), &r.raw_name, r.blob, r));
            }
        }
        rows.sort_by(|a, b| (a.0, a.1, a.2, a.3).cmp(&(b.0, b.1, b.2, b.3)));
        let overlay = overlay_from(&rows, &paths, &symbols);
        let same_path: Arc<[_]> =
            paths.iter().map(|(_, t)| self.rules.same_path_of(t)).collect::<Vec<_>>().into();
        let decls = declarations(&self.inputs);
        let mut index = ScanIndex {
            sources: self.sources,
            provenance: self.provenance,
            paths,
            symbols,
            overlay: Arc::new(overlay),
            same_path,
            decls: decls.into(),
            scope: self.rules.scope(),
            derived: Derived::default(),
            diags: Arc::new(self.diags),
        };
        index.derived = derive(&index);
        index
    }
}

fn overlay_from(
    rows: &[Row<'_>],
    paths: &crate::Interner,
    symbols: &crate::Interner,
) -> OverlayTable {
    let mut by_path: Vec<SmallVec<[Contribution; 2]>> = vec![SmallVec::new(); paths.len() as usize];
    let mut slot = 0u32;
    for group in rows.chunk_by(|a, b| (a.0, a.1) == (b.0, b.1)) {
        let (pid, src, raw, blob, rec) = group[0];
        by_path[pid as usize].push(Contribution {
            source: SourceId(src),
            blob,
            raw_name: SymbolId(symbols.find(raw).unwrap_or(0)),
            completeness: rec.completeness,
            ambiguous: group.len() > 1 || group.iter().any(|r| r.4.ambiguous),
            slot,
        });
        slot += 1;
    }
    // Slots must follow (PathId, SourceId) order, which `rows` already is.
    OverlayTable::new(by_path, slot)
}

fn declarations(inputs: &[SourceInput]) -> Vec<ReplaceDecl> {
    let mut v: Vec<ReplaceDecl> = inputs
        .iter()
        .flat_map(|i| {
            i.replace_paths.iter().map(|p| ReplaceDecl {
                declarer: i.source,
                prefix: normalise_prefix(p).into_boxed_str(),
            })
        })
        .filter(|d| !d.prefix.is_empty())
        .collect();
    v.sort_by(|a, b| (a.declarer.index(), &a.prefix).cmp(&(b.declarer.index(), &b.prefix)));
    v.dedup();
    v
}

/// Stage B: one pass over the overlay.
fn derive(index: &ScanIndex) -> Derived {
    let slots = index.overlay.slot_count();
    let mut d = Derived {
        surviving: BitSet::new(slots),
        possibly: BitSet::new(slots),
        ..Derived::default()
    };
    for p in index.overlay.paths() {
        let v = index.path_verdict(p);
        d.shadowed.extend(v.shadows.iter().copied());
        if let Some(w) = v.resolution.winner {
            mark(&mut d.surviving, index, p, &[w]);
        } else if !v.resolution.gaps.is_empty() {
            mark(&mut d.possibly, index, p, &v.live);
            d.path_gaps.insert(p, v.resolution.gaps.clone());
        }
    }
    d
}

fn mark(set: &mut BitSet, index: &ScanIndex, p: PathId, who: &[smd_core::Contender]) {
    for c in who {
        if let Some(slot) = index.overlay.slot_of(p, c.source) {
            set.set(slot);
        }
    }
}
