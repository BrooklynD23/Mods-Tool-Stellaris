//! The launcher database's `mods` table as discovery's source of mods nothing else found.

use super::LauncherRead;
use super::rows::ModRow;

/// What reading the `mods` table came to.
pub(crate) enum DbMods<'a> {
    /// Every `mods` row of a verified snapshot.
    Rows(&'a [ModRow]),
    /// Not read, and why. Ordinary when the launcher was never run.
    NotConsulted(String),
}

impl LauncherRead {
    /// The `mods` rows of the scan's one verified read-only snapshot (never the live file
    /// directly). Failures are reasons, not errors: the order chain reports them in its own
    /// words from the same read.
    pub(crate) fn db_mods(&self) -> DbMods<'_> {
        match &self.result {
            Ok(loaded) => DbMods::Rows(&loaded.verified.rows.mods),
            Err(None) => DbMods::NotConsulted("the file is not there".to_owned()),
            Err(Some(f)) => DbMods::NotConsulted(format!(
                "step {} failed on path {}: {}",
                f.fail.step, f.path, f.fail.detail
            )),
        }
    }
}
