//! How a mod came to be discovered.

/// Where a discovered mod was found. Each is a different kind of evidence about whether
/// the game will load it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum ModOrigin {
    /// A launcher `<user_dir>/mod/*.mod` file.
    ModFile,
    /// A workshop content directory no launcher file claimed.
    WorkshopDir,
    /// A `<user_dir>/mod/<dir>/` holding `descriptor.mod` that no `*.mod` file points at
    /// (copied in by hand). The launcher does not know it, so the game does not load it.
    UnregisteredDir,
    /// A `<user_dir>/mod/*.zip` no `archive=` names. Inventoried, content unread.
    LooseArchive,
    /// A row of the launcher database's `mods` table that no other source claimed.
    LauncherDbOnly,
}

impl ModOrigin {
    /// The stable tag reports and JSON carry.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            ModOrigin::ModFile => "mod_file",
            ModOrigin::WorkshopDir => "workshop_dir",
            ModOrigin::UnregisteredDir => "unregistered_dir",
            ModOrigin::LooseArchive => "loose_archive",
            ModOrigin::LauncherDbOnly => "launcher_db_only",
        }
    }
}
