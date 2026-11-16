//! `ModKey` derivation and de-duplication.

use std::collections::BTreeSet;

use smd_core::{Descriptor, ModKey};

use crate::codes::{self, IngestDiagnostic};

/// The key vanilla carries. No mod may take it.
pub(crate) const VANILLA_KEY: &str = "vanilla";

/// A stable hash of the fields that identify a mod when it has no workshop id.
///
/// Deliberately not the descriptor's `path=`, which is machine-specific for a mod that
/// moved between drives: the key must survive a move.
pub(super) fn hashed(desc: &Descriptor, dir_name: &str, tag: &str) -> ModKey {
    let mut h = blake3::Hasher::new();
    let mut field = |s: &str| {
        h.update(&(s.len() as u64).to_le_bytes());
        h.update(s.as_bytes());
    };
    field(tag);
    field(dir_name);
    field(&desc.name);
    field(desc.version.as_deref().unwrap_or(""));
    let archive = desc.archive.as_deref().and_then(|a| a.file_name());
    field(&archive.map(|a| a.to_string_lossy().into_owned()).unwrap_or_default());
    let hex = h.finalize().to_hex();
    ModKey(format!("h-{}", &hex.as_str()[..32]).into())
}

/// The key a descriptor derives: its workshop id when it has one, else a hash.
pub(super) fn derive(desc: &Descriptor, dir_name: &str, trusted: bool) -> ModKey {
    match desc.remote_file_id.as_deref() {
        Some(id) if trusted && !id.is_empty() => ModKey(id.into()),
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
