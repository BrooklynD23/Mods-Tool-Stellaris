//! Per-path extraction classification: what a parser and the key resolver need from a
//! `[[dir]]` entry (doc 06 § From events to facts, doc 10 § Keys, by resolution stage).

use serde::Deserialize;
use smd_core::{DomainId, ExtractionClass, FileKind, FileOrder, MergeMode};

use crate::profile::ProfileError;

/// The extraction keys of one `[[dir]]` entry, as written.
#[derive(Deserialize, Default, Clone)]
pub(crate) struct RawExtract {
    domain: Option<String>,
    merge: Option<String>,
    level: Option<u8>,
    extractor: Option<String>,
    parser: Option<String>,
    order: Option<String>,
    #[serde(default)]
    additive_children: Vec<String>,
}

impl RawExtract {
    pub(crate) fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }
}

/// The classification of one path: dispatch, extraction, and the key-layer behaviour.
///
/// A path under no `[[dir]]` is [`DirClass::default`]: `FileKind::Unknown`, no domain,
/// `MergeMode::Unknown`, and no filename order. Nothing is parsed for it and no key-level
/// outcome is stated about it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DirClass {
    /// Dispatch class.
    pub kind: FileKind,
    /// `FileParser::id()` the profile names for this path; empty when none.
    pub parser: &'static str,
    /// `"depth_key"`, `"pdx_event"` or `"none"`.
    pub extractor: &'static str,
    /// Which depth's keys are definitions.
    pub level: u8,
    /// Domain id: the domain's position in [`Profile::domain_names`](crate::Profile::domain_names).
    pub domain: Option<DomainId>,
    /// Domain name.
    pub domain_name: Option<Box<str>>,
    /// Cross-file behaviour.
    pub merge: MergeMode,
    /// Filename direction; `None` = unknown.
    pub order: Option<FileOrder>,
    /// Children that accumulate (`merge_by_key_additive_body` only).
    pub additive_children: Box<[Box<str>]>,
}

impl DirClass {
    /// The parse-cache classification term (H10).
    #[must_use]
    pub fn extraction_class(&self) -> ExtractionClass {
        ExtractionClass {
            kind: self.kind,
            parser: self.parser,
            extractor: self.extractor,
            level: self.level,
            domain: self.domain_name.clone(),
            additive_children: self.additive_children.clone(),
        }
    }
}

fn bad(key: &'static str, v: &str) -> ProfileError {
    ProfileError::Value { key, found: v.to_owned() }
}

/// `parser` → `(kind, parser id)`. Absent means the profile's script syntax.
fn dispatch(parser: Option<&str>) -> Result<(FileKind, &'static str), ProfileError> {
    match parser {
        None | Some("clausewitz") => Ok((FileKind::Script, "clausewitz")),
        Some("pdx_locyml") => Ok((FileKind::Localisation, "pdx_locyml")),
        Some("none") => Ok((FileKind::Asset, "none")),
        Some(other) => Err(bad("parser", other)),
    }
}

fn extractor(v: Option<&str>, kind: FileKind) -> Result<&'static str, ProfileError> {
    match v {
        _ if kind == FileKind::Asset => Ok("none"),
        None | Some("depth_key") => Ok("depth_key"),
        Some("pdx_event") => Ok("pdx_event"),
        Some("none") => Ok("none"),
        Some(other) => Err(bad("extractor", other)),
    }
}

fn merge(v: Option<&str>) -> Result<MergeMode, ProfileError> {
    match v {
        None | Some("unknown") => Ok(MergeMode::Unknown),
        Some("path_only") => Ok(MergeMode::PathOnly),
        Some("merge_by_key") => Ok(MergeMode::MergeByKey),
        Some("merge_by_key_additive_body") => Ok(MergeMode::MergeByKeyAdditiveBody),
        Some("additive") => Ok(MergeMode::Additive),
        Some("single") => Ok(MergeMode::Single),
        Some(other) => Err(bad("merge", other)),
    }
}

/// `lios` / `fios` / `unknown`, as the profile writes a filename direction.
pub(crate) fn order(key: &'static str, v: &str) -> Result<Option<FileOrder>, ProfileError> {
    match v {
        "lios" => Ok(Some(FileOrder::Lios)),
        "fios" => Ok(Some(FileOrder::Fios)),
        "unknown" => Ok(None),
        other => Err(bad(key, other)),
    }
}

impl RawExtract {
    /// Type the entry. `domains` is the profile's sorted domain list; `default_order` is
    /// `[load_order] default_order`.
    pub(crate) fn typed(
        &self,
        domains: &[Box<str>],
        default_order: Option<FileOrder>,
    ) -> Result<DirClass, ProfileError> {
        let (kind, parser) = dispatch(self.parser.as_deref())?;
        let extractor = extractor(self.extractor.as_deref(), kind)?;
        let merge = merge(self.merge.as_deref())?;
        let additive: Box<[Box<str>]> =
            self.additive_children.iter().map(|c| c.as_str().into()).collect();
        // Invariant A1 (doc 03, H4): the additive mode is selectable only where the
        // extractor emits `AdditiveSplit`, which `depth_key` does when children are named.
        let additive_ok = extractor == "depth_key" && !additive.is_empty();
        if merge == MergeMode::MergeByKeyAdditiveBody && !additive_ok {
            return Err(bad("merge", "merge_by_key_additive_body without additive_children"));
        }
        if !additive.is_empty() && merge != MergeMode::MergeByKeyAdditiveBody {
            return Err(bad("additive_children", "set without merge_by_key_additive_body"));
        }
        let order = match self.order.as_deref() {
            Some(v) => order("order", v)?,
            None => default_order,
        };
        let domain = self.domain.as_deref().and_then(|d| {
            let i = domains.binary_search_by(|x| (**x).cmp(d)).ok()?;
            Some(DomainId(u16::try_from(i).ok()?))
        });
        Ok(DirClass {
            kind,
            parser,
            extractor,
            level: self.level.unwrap_or(0),
            domain,
            domain_name: self.domain.as_deref().map(Into::into),
            merge,
            order,
            additive_children: additive,
        })
    }
}

/// What the filename axis compares (`[load_order] filename_key`).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FilenameKey {
    /// The base name without its extension.
    BasenameNoExt,
    /// The base name.
    Basename,
    /// The path relative to the directory.
    DirRelative,
}

/// The Stage D parameters of `[load_order]`; `None` = `unknown`, which the resolver
/// enumerates.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub struct KeyReadings {
    /// `filename_key`.
    pub filename_key: Option<FilenameKey>,
    /// `vanilla_in_filename_axis`.
    pub vanilla_in_axis: Option<bool>,
    /// `in_file_duplicate`: `Some(true)` = last wins.
    pub in_file_last: Option<bool>,
}

/// The `[load_order]` keys behind [`KeyReadings`] and `default_order`, as written.
#[derive(Deserialize, Default, Clone)]
pub(crate) struct RawReadings {
    filename_key: Option<String>,
    vanilla_in_filename_axis: Option<toml::Value>,
    in_file_duplicate: Option<String>,
    default_order: Option<String>,
}

impl RawReadings {
    pub(crate) fn typed(&self) -> Result<(KeyReadings, Option<FileOrder>), ProfileError> {
        let filename_key = match self.filename_key.as_deref() {
            None | Some("unknown") => None,
            Some("basename_noext") => Some(FilenameKey::BasenameNoExt),
            Some("basename") => Some(FilenameKey::Basename),
            Some("dir_relative") => Some(FilenameKey::DirRelative),
            Some(other) => return Err(bad("filename_key", other)),
        };
        let vanilla_in_axis = match &self.vanilla_in_filename_axis {
            None => None,
            Some(toml::Value::Boolean(b)) => Some(*b),
            Some(toml::Value::String(s)) if s == "unknown" => None,
            Some(other) => return Err(bad("vanilla_in_filename_axis", &other.to_string())),
        };
        let in_file_last = match self.in_file_duplicate.as_deref() {
            None | Some("unknown") => None,
            Some("first") => Some(false),
            Some("last") => Some(true),
            Some(other) => return Err(bad("in_file_duplicate", other)),
        };
        let default_order =
            order("default_order", self.default_order.as_deref().unwrap_or("lios"))?;
        Ok((KeyReadings { filename_key, vanilla_in_axis, in_file_last }, default_order))
    }
}
