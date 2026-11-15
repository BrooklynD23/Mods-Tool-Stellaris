//! `ModKey` derivation and de-duplication.

use std::collections::BTreeSet;

use smd_core::{Descriptor, ModKey};

use crate::codes::{self, IngestDiagnostic};

/// The key vanilla carries. No mod may take it.
pub(crate) const VANILLA_KEY: &str = "vanilla";

/// A stable hash of the fields that identify a mod when it has no workshop id.
///
/// Deliberately not the descriptor's `path=`, which is machine-specific for a mod that
/// moved between drives: the key must survive a move. Nor its `version`: a version bump
/// must not re-key the mod and orphan whatever was recorded against the old key.
pub(super) fn hashed(desc: &Descriptor, dir_name: &str, tag: &str) -> ModKey {
    let mut h = blake3::Hasher::new();
    let mut field = |s: &str| {
        h.update(&(s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    };
    field(tag);
    field(dir_name);
    field(&desc.name);
    let archive = desc.archive.as_deref().and_then(|a| a.file_name());
    field(&archive.map(|a| a.to_string_lossy().into_owned()).unwrap_or_default());
    let hex = h.finalize().to_hex();
    ModKey(format!("h-{}", &hex.as_str()[..32]).into())
}

/// Which platform an id belongs to. Ids from different platforms may collide as numbers,
/// so a key always carries its platform: `steam:<id>`, `pdx:<id>`, or `h-<hash>`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(super) enum Provider {
    Steam,
    Paradox,
}

impl Provider {
    fn key(self, id: &str) -> ModKey {
        let prefix = match self {
            Provider::Steam => "steam",
            Provider::Paradox => "pdx",
        };
        ModKey(format!("{prefix}:{id}").into())
    }
}

/// Which platform a launcher `*.mod` file's descriptor id belongs to.
///
/// Rule (doc 05, "Mod sources and origin"), strongest evidence first: a launcher database
/// row (`steamId` is Steam, else `pdxId` is Paradox; decided in `launcher_db`), the `pdx_`
/// / `ugc_` mod-file name prefix, a workshop directory (Steam). With none of those the id
/// is read as Steam-compatible, `steam:<id>`: never `pdx:` without evidence, so a Paradox
/// id can neither be mistaken for a Steam one nor collide with it.
pub(super) fn provider_of_file(file: &str) -> Provider {
    if file.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("pdx_")) {
        Provider::Paradox
    } else {
        Provider::Steam
    }
}

/// An id with the platform that issued it.
pub(super) type PlatformId = (Provider, Box<str>);

/// The key a candidate derives: its platform id when it has one (explicit evidence, else
/// the descriptor's id read as Steam), else a hash.
pub(super) fn derive(
    ids: Option<&PlatformId>,
    desc: &Descriptor,
    dir_name: &str,
    trusted: bool,
) -> ModKey {
    let from_desc = || desc.remote_file_id.as_deref().map(|id| (Provider::Steam, id));
    let explicit = ids.map(|(p, id)| (*p, id.as_ref()));
    match explicit.or_else(|| trusted.then(from_desc).flatten()) {
        Some((p, id)) if !id.is_empty() => p.key(id),
        _ => hashed(desc, dir_name, if trusted { "d" } else { "unread" }),
    }
}

/// Claims keys in a fixed order and re-keys a repeat.
#[derive(Default)]
pub(super) struct Claims {
    seen: BTreeSet<Box<str>>,
}

impl Claims {
    /// A claim set with vanilla's key already taken.
    pub(super) fn new() -> Claims {
        let mut c = Claims::default();
        c.seen.insert(VANILLA_KEY.into());
        c
    }

    /// Take `key`, or a deterministic replacement if it is taken; the diagnostic says so.
    pub(super) fn claim(
        &mut self,
        key: ModKey,
        desc: &Descriptor,
        dir_name: &str,
    ) -> (ModKey, Option<IngestDiagnostic>) {
        if self.seen.insert(key.0.clone()) {
            return (key, None);
        }
        let mut n = 2u32;
        loop {
            let tag = format!("dup{n}");
            let alt = hashed(desc, dir_name, &tag);
            if self.seen.insert(alt.0.clone()) {
                let why = format!("key `{key}` was already taken; this mod is keyed `{alt}`");
                return (alt, Some(IngestDiagnostic::warn(codes::DUPLICATE_KEY, dir_name, why)));
            }
            n += 1;
        }
    }
}
