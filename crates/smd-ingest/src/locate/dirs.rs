//! The user data directory and the workshop roots.

use std::path::{Path, PathBuf};

use smd_fs::{Origin, Os};

use super::{Run, push_unique};

impl Run<'_> {
    /// Fill a profile template. `{DOCUMENTS}`, `{XDG_DATA_HOME}` and `{HOME}` come from the
    /// machine; `{STEAM_LIB}` and `{PROTON_PREFIX}` need `lib`. `None` when a placeholder
    /// cannot be filled on this machine.
    pub(crate) fn expand_path(
        &self,
        template: &str,
        docs: Option<&Path>,
        lib: Option<&Path>,
    ) -> Option<PathBuf> {
        let path = |p: Option<PathBuf>| p.map(|p| p.display().to_string());
        let proton = |lib: &Path| {
            let id = self.profile.steam_id()?;
            Some(lib.join("steamapps").join("compatdata").join(id.to_string()).join("pfx"))
        };
        let var = |name: &str| match name {
            "DOCUMENTS" => path(docs.map(Path::to_path_buf)),
            "XDG_DATA_HOME" => path(smd_fs::xdg_data_home(self.env)),
            "HOME" => path(smd_fs::home_dir(self.env)),
            "STEAM_LIB" => path(lib.map(Path::to_path_buf)),
            "PROTON_PREFIX" => path(lib.and_then(proton)),
            _ => None,
        };
        smd_profiles::expand(template, &var).ok().map(PathBuf::from)
    }

    pub(crate) fn user_dir(&mut self, flag: Option<&Path>, libs: &[PathBuf]) -> Option<PathBuf> {
        if let Some(p) = flag {
            return self.pinned("user_dir", p, Origin::Flag);
        }
        if let Some(v) = self.env.var("SMD_USER_DIR") {
            if let Some(p) = self.pinned("user_dir", Path::new(&v), Origin::Env) {
                return Some(p);
            }
        }
        let found = self.user_dir_templates().or_else(|| self.proton_user_dir(libs));
        if found.is_none() {
            self.unresolved("user_dir");
        }
        found
    }

    /// The OS template, then the profile's extra candidates. `{DOCUMENTS}` is `Registry`
    /// when the registry named it, else `Default`.
    fn user_dir_templates(&mut self) -> Option<PathBuf> {
        let docs = smd_fs::documents_dirs(self.env);
        let mut templates: Vec<String> = self
            .profile
            .locations()
            .user_dir
            .get(self.env.os().key())
            .cloned()
            .into_iter()
            .collect();
        templates.extend(self.profile.stores().extra_user_dirs.clone());
        for t in templates {
            // A template that names Documents is tried once per Documents candidate
            // (registry first, then the profile folder); any other is tried once.
            let bases: Vec<Option<(PathBuf, Origin)>> = if t.contains("{DOCUMENTS}") {
                docs.iter().cloned().map(Some).collect()
            } else {
                vec![None]
            };
            for base in bases {
                let (dir, origin) = match &base {
                    Some((p, o)) => (Some(p.as_path()), *o),
                    None => (None, Origin::Default),
                };
                if let Some(p) = self.expand_path(&t, dir, None) {
                    if self.try_dir("user_dir", p.clone(), origin) {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    /// A Wine prefix inside a Steam library (Linux, the Windows build under Proton).
    fn proton_user_dir(&mut self, libs: &[PathBuf]) -> Option<PathBuf> {
        let template = self.profile.locations().user_dir.get("proton").cloned()?;
        if self.env.os() != Os::Linux {
            return None;
        }
        for lib in libs {
            if let Some(p) = self.expand_path(&template, None, Some(lib)) {
                if self.try_dir("user_dir", p.clone(), Origin::Steam) {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Flags win outright. Otherwise every Steam library that has the workshop content
    /// directory contributes one: the workshop may live apart from the game.
    pub(crate) fn workshop_dirs(&mut self, flags: &[PathBuf], libs: &[PathBuf]) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for f in flags {
            if let Some(p) = self.pinned("workshop_dir", f, Origin::Flag) {
                push_unique(&mut out, p);
            }
        }
        if !flags.is_empty() {
            return out;
        }
        let template = self.profile.locations().workshop.clone();
        for lib in libs {
            let path = template.as_deref().and_then(|t| self.expand_path(t, None, Some(lib)));
            if let Some(p) = path {
                if self.try_dir("workshop_dir", p.clone(), Origin::Steam) {
                    push_unique(&mut out, p);
                }
            }
        }
        if out.is_empty() {
            self.unresolved("workshop_dir");
        }
        out
    }
}
