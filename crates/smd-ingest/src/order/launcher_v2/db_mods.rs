//! The launcher database's `mods` table as discovery's source of mods nothing else found.

use smd_core::ResolvedLocations;

use super::LauncherV2Sqlite;
use super::rows::ModRow;

/// What reading the `mods` table came to.
pub(crate) enum DbMods {
    /// Every `mods` row of a verified snapshot.
    Rows(Vec<ModRow>),
    /// Not read, and why. Ordinary when the launcher was never run.
    NotConsulted(String),
}

impl LauncherV2Sqlite {
    /// Every `mods` row, through the same read-only verified snapshot the playset reader
    /// uses (never the live file directly). Failures are reasons, not errors: the order
    /// chain reports them in its own words when it reads the same file.
    pub(crate) fn db_mods(&self, loc: &ResolvedLocations) -> DbMods {
        let (mut temp, mut build, mut diags) = (None, String::new(), Vec::new());
        match self.read(loc, &mut temp, &mut build, &mut diags) {
            Ok(loaded) => DbMods::Rows(loaded.verified.rows.mods),
            Err(None) => DbMods::NotConsulted("the file is not there".to_owned()),
            Err(Some(f)) => DbMods::NotConsulted(format!(
                "step {} failed on path {}: {}",
                f.fail.step, f.path, f.fail.detail
            )),
        }
    }
}
