//! Playset metadata the `dep.*` and `meta.*` rules read: descriptors and the game version.
//!
//! Dependencies themselves live on [`SourceMeta::deps`](smd_core::SourceMeta::deps); this
//! holds what the source table does not.

use smd_core::{Descriptor, GameVersion, ResolvedDependency, SourceId, SourceTable};

/// Both descriptor copies of one source.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ModDescriptors {
    /// The copy that decides whether and where the mod loads: the launcher `*.mod` when
    /// there is one, otherwise the content directory's own `descriptor.mod`.
    pub declared: Descriptor,
    /// The content directory's `descriptor.mod`, read only when it is a *second* copy (the
    /// mod came from a launcher `*.mod`) and the content root is readable. `None` is no
    /// claim: absent, unreadable, or not parsed cleanly.
    pub inner: Option<Descriptor>,
}

/// Scan-wide metadata. `Default` claims nothing: no version, no descriptors.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PlaysetMeta {
    /// The installed game version; `None` when it could not be read or parsed.
    pub game_version: Option<GameVersion>,
    /// Index-parallel to the source table; may be shorter (a missing row is no claim).
    pub descriptors: Vec<ModDescriptors>,
}

impl PlaysetMeta {
    /// The descriptors of `s`, when known.
    #[must_use]
    pub fn descriptors_of(&self, s: SourceId) -> Option<&ModDescriptors> {
        self.descriptors.get(s.index() as usize)
    }

    /// Every string a rule may cite as evidence, for the symbol interner.
    pub(crate) fn strings<'a>(
        &'a self,
        table: &'a SourceTable,
    ) -> impl Iterator<Item = String> + 'a {
        let deps = table.iter().flat_map(|(_, m)| m.deps.iter()).map(|d| match d {
            ResolvedDependency::Resolved { declared, .. }
            | ResolvedDependency::Missing { declared, .. } => declared.to_string(),
            _ => String::new(),
        });
        let descs = self
            .descriptors
            .iter()
            .flat_map(|d| std::iter::once(&d.declared).chain(&d.inner))
            .flat_map(descriptor_strings);
        let gv = self.game_version.iter().map(version_text);
        deps.chain(descs).chain(gv)
    }
}

fn descriptor_strings(d: &Descriptor) -> Vec<String> {
    let mut v = vec![d.name.to_string()];
    v.extend(d.version.iter().map(ToString::to_string));
    v.extend(d.remote_file_id.iter().map(ToString::to_string));
    v.extend(d.supported_version.iter().map(pattern_text));
    v
}

/// `4.0.*` from its components; the text form rules cite.
#[must_use]
pub fn pattern_text(p: &smd_core::VersionPattern) -> String {
    let parts: Vec<String> =
        p.0.iter()
            .map(|c| match c {
                smd_core::VersionComponent::Exact(n) => n.to_string(),
                smd_core::VersionComponent::Wildcard => "*".to_owned(),
            })
            .collect();
    parts.join(".")
}

/// `4.0.3` from its components.
#[must_use]
pub fn version_text(v: &GameVersion) -> String {
    let parts: Vec<String> = v.0.iter().map(ToString::to_string).collect();
    parts.join(".")
}
