//! The game install directory, from a flag down to a store's default.

use std::path::{Path, PathBuf};

use smd_fs::{Origin, Os};

use super::steam::read_capped;
use super::{Run, Store};
use crate::codes::{self, IngestDiagnostic};

/// Where installers write uninstall entries (an OS fact, not a game's).
const UNINSTALL_ROOTS: [&str; 3] = [
    r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
    r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
];

/// Whether `s` can be one directory name under `steamapps/common`.
fn is_plain_name(s: &str) -> bool {
    !s.is_empty() && s != ".." && s != "." && !s.contains(['/', '\\'])
}

impl Run<'_> {
    /// An override: `Some` only when it exists; a missing one is a diagnostic.
    pub(crate) fn pinned(
        &mut self,
        key: &'static str,
        p: &Path,
        origin: Origin,
    ) -> Option<PathBuf> {
        if self.try_dir(key, p.to_path_buf(), origin) {
            return Some(p.to_path_buf());
        }
        self.missing_override(key, origin, p);
        None
    }

    pub(crate) fn game_dir(
        &mut self,
        flag: Option<&Path>,
        libs: &[PathBuf],
    ) -> (Option<PathBuf>, Store) {
        if let Some(p) = flag {
            let found = self.pinned("game_dir", p, Origin::Flag);
            return (found.clone(), if found.is_some() { Store::Other } else { Store::Unknown });
        }
        if let Some(v) = self.env.var("SMD_GAME_DIR") {
            if let Some(p) = self.pinned("game_dir", Path::new(&v), Origin::Env) {
                return (Some(p), Store::Other);
            }
        }
        let found = self.steam_game(libs).map(|p| (p, Store::Steam));
        let found = found.or_else(|| self.gog_game().map(|p| (p, Store::Gog)));
        let found = found.or_else(|| self.xbox_game().map(|p| (p, Store::Xbox)));
        match found {
            Some((p, store)) => (Some(p), store),
            None => {
                self.unresolved("game_dir");
                (None, Store::Unknown)
            }
        }
    }

    /// `appmanifest_<id>.acf` names the install directory when Steam wrote one; the
    /// profile's template is the fallback for a library without a manifest.
    fn steam_game(&mut self, libs: &[PathBuf]) -> Option<PathBuf> {
        let template = self.profile.locations().install.clone();
        for lib in libs {
            let named =
                self.manifest_install_dir(lib).map(|d| lib.join("steamapps/common").join(d));
            let path = named.or_else(|| self.expand_path(template.as_deref()?, None, Some(lib)));
            if let Some(p) = path {
                if self.try_dir("game_dir", p.clone(), Origin::Steam) {
                    return Some(p);
                }
            }
        }
        None
    }

    fn manifest_install_dir(&mut self, lib: &Path) -> Option<String> {
        let id = self.profile.steam_id()?;
        let file = lib.join("steamapps").join(format!("appmanifest_{id}.acf"));
        let parsed = match read_capped(&file) {
            Ok(None) => return None,
            Ok(Some(text)) => smd_fs::parse_install_dir(&text).map_err(|e| e.to_string()),
            Err(e) => Err(e),
        };
        match parsed {
            Ok(dir) => dir.filter(|d| is_plain_name(d)),
            Err(why) => {
                let subject = file.display().to_string();
                self.diags.push(IngestDiagnostic::warn(codes::LOCATE_VDF_INVALID, &subject, why));
                None
            }
        }
    }

    /// A directory that holds the profile's executable (when it names one).
    fn has_game(&self, dir: &Path) -> bool {
        let exe = self.profile.stores().executable.as_deref();
        dir.is_dir() && exe.is_none_or(|e| dir.join(e).is_file())
    }

    /// An uninstall entry whose `DisplayName` starts with the profile's prefix and whose
    /// `InstallLocation` holds the executable.
    fn gog_game(&mut self) -> Option<PathBuf> {
        let prefix = self.profile.stores().gog_display_name_prefix.clone()?.to_lowercase();
        if self.env.os() != Os::Windows {
            return None;
        }
        for root in UNINSTALL_ROOTS {
            for entry in self.env.registry(root, Some("DisplayName"), true) {
                if !entry.data.to_lowercase().starts_with(&prefix) {
                    continue;
                }
                let at = self.env.registry(&entry.key, Some("InstallLocation"), false);
                let Some(loc) = at.into_iter().map(|v| v.data).find(|d| !d.is_empty()) else {
                    continue;
                };
                let path = PathBuf::from(loc);
                let ok = self.has_game(&path);
                self.record("game_dir", path.clone(), Origin::Registry, ok);
                if ok {
                    return Some(path);
                }
            }
        }
        None
    }

    fn xbox_game(&mut self) -> Option<PathBuf> {
        let rel = self.profile.stores().xbox_install.clone()?;
        for drive in self.env.drive_roots() {
            let path = drive.join(&rel);
            let ok = self.has_game(&path);
            self.record("game_dir", path.clone(), Origin::Default, ok);
            if ok {
                return Some(path);
            }
        }
        None
    }
}
