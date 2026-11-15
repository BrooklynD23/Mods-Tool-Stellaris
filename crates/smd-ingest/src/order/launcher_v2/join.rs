//! Joining launcher `mods` rows to discovered mods.
//!
//! One rule, used both to decide a row is already found (discovery) and to place a playset
//! entry on a mod (playset). Keys are tried in this order, the first to hit wins:
//! `gameRegistryId` (its file name is the mod's `*.mod` file), `steamId` (against `steam:`
//! ids), `pdxId` (against `pdx:` ids), then `dirPath` / `archivePath` by host path
//! identity. Every column is optional; an absent or empty one is not tried. Ids are
//! compared with their platform: a `pdxId` never matches a Steam id of the same digits.
//!
//! **Unverified against a real launcher:** that `pdxId` names Paradox Mods downloads, and
//! that their descriptors carry that id as `remote_file_id`, are assumptions (VERIFY V03).
//! A Paradox mod whose file name and row give no such evidence is `steam:`-keyed and found
//! by `dirPath` instead.

use std::path::{Path, PathBuf};

use super::rows::ModRow;

/// What a discovered mod can be recognised by.
#[derive(Clone, Default, Debug)]
pub(crate) struct Ident {
    pub mod_file: Option<String>,
    /// The platform-qualified id: `steam:<id>` or `pdx:<id>` (see [`remote`]).
    pub remote_id: Option<String>,
    /// Every path the mod stands for: its content directory, its resolved `path=`/
    /// `archive=`.
    pub paths: Vec<PathBuf>,
}

/// A platform-qualified id, the form [`Ident::remote_id`] holds.
pub(crate) fn remote(paradox: bool, id: &str) -> String {
    format!("{}:{id}", if paradox { "pdx" } else { "steam" })
}

/// `mod/ugc_1.mod` and `mod\ugc_1.mod` both name `ugc_1.mod`.
fn file_name(listed: &str) -> &str {
    listed.rsplit(['/', '\\']).next().unwrap_or(listed)
}

/// Whether the launcher's `declared` path (absolute, or relative to `base`) is `to`.
/// Compared as written, then with links resolved: the launcher writes `D:/Steam/..` where
/// discovery holds the canonical form.
fn same_path(declared: &str, base: &Path, to: &Path) -> bool {
    let abs = base.join(declared);
    smd_fs::same_host_dir(&abs, to)
        || std::fs::canonicalize(&abs).is_ok_and(|c| smd_fs::same_host_dir(&c, to))
}

/// The index of the mod `row` stands for, if any.
pub(crate) fn find(row: &ModRow, idents: &[Ident], base: &Path) -> Option<usize> {
    let pos = |hit: &dyn Fn(&Ident) -> bool| idents.iter().position(hit);
    let by_id = |want: Option<&str>| {
        let want = want?;
        pos(&|i| i.remote_id.as_deref() == Some(want))
    };
    let by_file = || {
        let want = file_name(row.game_registry_id.as_deref()?);
        pos(&|i| i.mod_file.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(want)))
    };
    let by_path = || {
        let declared = [row.dir_path.as_deref(), row.archive_path.as_deref()];
        declared
            .into_iter()
            .flatten()
            .find_map(|d| pos(&|i: &Ident| i.paths.iter().any(|p| same_path(d, base, p))))
    };
    by_file()
        .or_else(|| by_id(row.steam_id.as_deref().map(|i| remote(false, i)).as_deref()))
        .or_else(|| by_id(row.pdx_id.as_deref().map(|i| remote(true, i)).as_deref()))
        .or_else(by_path)
}
