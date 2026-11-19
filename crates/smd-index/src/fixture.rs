//! `PlaysetBuilder` — synthetic playsets for tests and rule cases.
//!
//! Lives here rather than in `smd-testkit` because it must construct a [`ScanIndex`], and
//! `smd-testkit` (layer 1) cannot depend on this crate (layer 3). Nothing here reads a
//! filesystem or names a game.

use smd_core::{
    Activation, Blob, Confidence, DomainId, FactPayload, FileOrder, Hasher128, MergeMode, ModKey,
    ParseCompleteness, PlaysetProvenance, RawFact, SamePath, SnapshotMethod, SourceId, SourceMeta,
    SourceRole, SourceTable, Span, UnparsedReason,
};
use smd_fs::{NormPath, shipped_name};
use smd_profiles::{FilenameKey, KeyReadings};

use crate::build::{FileRecord, IndexBuilder, SourceInput};
use crate::defs::{PathClass, SymbolRules};
use crate::index::ScanIndex;
use crate::overlay_rules::{OverlayRules, ReplacePathScope};

/// Content address of `content`, in the form walk records carry.
#[must_use]
pub fn blob_of(content: &str) -> Blob {
    let mut h = Hasher128::new();
    h.write(content.as_bytes());
    Blob { hash: h.finish().0.to_le_bytes(), len: content.len() as u64 }
}

/// How a fixture file was read.
#[derive(Copy, Clone, Debug)]
pub struct FileShape {
    /// Content address, when established.
    pub blob: Option<Blob>,
    /// How completely it was read.
    pub completeness: ParseCompleteness,
    /// Another entry of the same source has the same identity key.
    pub ambiguous: bool,
}

impl FileShape {
    /// A hashed file with these contents.
    #[must_use]
    pub fn of(content: &str) -> FileShape {
        FileShape {
            blob: Some(blob_of(content)),
            completeness: ParseCompleteness::NotApplicable,
            ambiguous: false,
        }
    }

    /// A file whose bytes were never established.
    #[must_use]
    pub fn unread(why: UnparsedReason) -> FileShape {
        FileShape { blob: None, completeness: ParseCompleteness::Unparsed(why), ambiguous: false }
    }

    /// Mark it as one of two entries sharing an identity key.
    #[must_use]
    pub fn ambiguous(self) -> FileShape {
        FileShape { ambiguous: true, ..self }
    }

    /// Replace the completeness.
    #[must_use]
    pub fn completeness(self, completeness: ParseCompleteness) -> FileShape {
        FileShape { completeness, ..self }
    }
}

/// Builds a [`ScanIndex`] from a handful of calls. Source 0 is vanilla, `Active`, rank `[0]`.
pub struct PlaysetBuilder {
    metas: Vec<SourceMeta>,
    inputs: Vec<SourceInput>,
    same_path: SamePath,
    scope: ReplacePathScope,
    order: Confidence,
    facts: Vec<(SourceId, NormPath, Vec<RawFact>)>,
    class: PathClass,
    readings: KeyReadings,
    incomplete: Vec<SourceId>,
}

/// The one domain fixture definitions belong to.
pub const FIXTURE_DOMAIN: &str = "thing";

impl Default for PlaysetBuilder {
    fn default() -> Self {
        PlaysetBuilder::new()
    }
}

impl PlaysetBuilder {
    /// A playset holding only vanilla; `same_path = Replace`, scope `Unknown`, order `Exact`.
    #[must_use]
    pub fn new() -> PlaysetBuilder {
        let mut b = PlaysetBuilder {
            metas: Vec::new(),
            inputs: Vec::new(),
            same_path: SamePath::Replace,
            scope: ReplacePathScope::Unknown,
            order: Confidence::Exact,
            facts: Vec::new(),
            class: PathClass {
                domain: Some(DomainId(0)),
                merge: MergeMode::MergeByKey,
                order: Some(FileOrder::Lios),
            },
            readings: KeyReadings {
                filename_key: Some(FilenameKey::Basename),
                vanilla_in_axis: Some(false),
                in_file_last: Some(true),
            },
            incomplete: Vec::new(),
        };
        b.push(Self::meta("vanilla", "Vanilla", vec![0], Activation::Active, SourceRole::Vanilla));
        b
    }

    fn push(&mut self, meta: SourceMeta) -> SourceId {
        let id = SourceId(u32::try_from(self.metas.len()).unwrap_or(u32::MAX));
        self.metas.push(meta);
        self.inputs.push(SourceInput {
            source: id,
            records: Vec::new(),
            replace_paths: Vec::new(),
        });
        id
    }

    fn meta(
        key: &str,
        name: &str,
        rank: Vec<u32>,
        activation: Activation,
        role: SourceRole,
    ) -> SourceMeta {
        SourceMeta {
            key: ModKey(key.into()),
            name: name.into(),
            rank,
            activation,
            role,
            deps: Vec::new(),
        }
    }

    /// Add a mod. `rank` empty means the order is unknown.
    pub fn add_mod(&mut self, name: &str, rank: &[u32], activation: Activation) -> SourceId {
        self.push(Self::meta(
            &format!("mod:{name}"),
            name,
            rank.to_vec(),
            activation,
            SourceRole::Local,
        ))
    }

    /// Add an official DLC source (`SourceRole::Dlc`).
    pub fn add_dlc(&mut self, name: &str, rank: &[u32], activation: Activation) -> SourceId {
        self.push(Self::meta(
            &format!("dlc:{name}"),
            name,
            rank.to_vec(),
            activation,
            SourceRole::Dlc,
        ))
    }

    /// Ship a file whose content is `content`.
    pub fn file(&mut self, s: SourceId, path: &str, content: &str) -> &mut Self {
        self.file_with(s, path, FileShape::of(content))
    }

    /// Ship a file with an explicit blob, completeness, and identity-collision flag.
    pub fn file_with(&mut self, s: SourceId, path: &str, shape: FileShape) -> &mut Self {
        let norm = NormPath::new(path).expect("fixture path must be valid");
        let raw = shipped_name(path.rsplit(['/', '\\']).next().unwrap_or(path));
        let rec = FileRecord {
            path: norm,
            raw_name: raw,
            blob: shape.blob,
            completeness: shape.completeness,
            ambiguous: shape.ambiguous,
        };
        self.inputs[s.index() as usize].records.push(rec);
        self
    }

    /// Declare a `replace_path` prefix for `s`.
    pub fn replace_path(&mut self, s: SourceId, prefix: &str) -> &mut Self {
        self.inputs[s.index() as usize].replace_paths.push(prefix.into());
        self
    }

    /// Ship a fully parsed file defining `(symbol, body)` pairs in [`FIXTURE_DOMAIN`].
    pub fn define(&mut self, s: SourceId, path: &str, defs: &[(&str, &str)]) -> &mut Self {
        let content: String = defs.iter().map(|(k, b)| format!("{k}={b};")).collect();
        let shape = FileShape::of(&content).completeness(ParseCompleteness::Complete);
        self.file_with(s, path, shape);
        let facts = defs
            .iter()
            .map(|(symbol, body)| RawFact {
                span: Span::default(),
                payload: FactPayload::Definition {
                    domain: DomainId(0),
                    symbol: (*symbol).into(),
                    body_hash: blob_of(body).hash,
                },
            })
            .collect();
        self.facts.push((s, NormPath::new(path).expect("fixture path must be valid"), facts));
        self
    }

    /// Set every path's merge mode and filename direction.
    pub fn key_layer(&mut self, merge: MergeMode, order: Option<FileOrder>) -> &mut Self {
        self.class = PathClass { merge, order, ..self.class };
        self
    }

    /// Set the Stage D readings (default: basename key, vanilla off the axis, last wins).
    pub fn readings(&mut self, r: KeyReadings) -> &mut Self {
        self.readings = r;
        self
    }

    /// Mark `s` content-incomplete, as a refused directory link would (H50).
    pub fn content_incomplete(&mut self, s: SourceId) -> &mut Self {
        self.incomplete.push(s);
        self
    }

    /// Set the uniform `same_path` behaviour.
    pub fn same_path(&mut self, sp: SamePath) -> &mut Self {
        self.same_path = sp;
        self
    }

    /// Set the `replace_path_scope` reading.
    pub fn replace_scope(&mut self, scope: ReplacePathScope) -> &mut Self {
        self.scope = scope;
        self
    }

    /// The `DirectoryScan` fallback: every mod's rank is empty and its activation `Unknown`.
    pub fn directory_scan(&mut self) -> &mut Self {
        for m in self.metas.iter_mut().skip(1) {
            m.rank.clear();
            m.activation = Activation::Unknown;
        }
        self.order = Confidence::Fallback;
        self
    }

    /// The source table as it stands.
    #[must_use]
    pub fn source_table(&self) -> SourceTable {
        SourceTable::new(self.metas.clone())
    }

    /// Inputs in a caller-chosen order, for order-independence tests.
    #[must_use]
    pub fn build_with(self, order: &[usize], reverse_files: bool) -> ScanIndex {
        let provenance = PlaysetProvenance {
            order_source: "fixture",
            order: self.order,
            activation: self.order,
            snapshot: SnapshotMethod::NotApplicable,
        };
        let rules = OverlayRules::uniform(self.scope, self.same_path);
        let mut b = IndexBuilder::new(SourceTable::new(self.metas), provenance, rules);
        let class = self.class;
        let domains = vec![FIXTURE_DOMAIN.into()];
        b.symbol_rules(SymbolRules::new(move |_| class, self.readings, domains));
        b.content_incomplete(self.incomplete);
        for (s, p, f) in self.facts {
            b.add_facts(s, &p, f);
        }
        for &i in order {
            let mut input = self.inputs[i].clone();
            if reverse_files {
                input.records.reverse();
            }
            b.add_source(input);
        }
        b.build()
    }

    /// Build the index.
    #[must_use]
    pub fn build(self) -> ScanIndex {
        let order: Vec<usize> = (0..self.inputs.len()).collect();
        self.build_with(&order, false)
    }
}
