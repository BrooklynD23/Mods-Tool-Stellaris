//! Fixture builders shared by the unit tests. Nothing here needs a game install.

use std::fs;
use std::path::PathBuf;

use smd_core::ResolvedLocations;
use tempfile::TempDir;

use crate::{DiscoveryOptions, KEY_INSTALL, KEY_USER_DIR, KEY_WORKSHOP};

/// A temporary world: a game dir, a user dir with `mod/`, a workshop dir, and a home.
pub(crate) struct World {
    pub tmp: TempDir,
    pub install: PathBuf,
    pub user: PathBuf,
    pub workshop: PathBuf,
    pub home: PathBuf,
}

impl World {
    pub(crate) fn new() -> World {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonical");
        let w = World {
            install: root.join("game"),
            user: root.join("user"),
            workshop: root.join("workshop"),
            home: root.join("home"),
            tmp,
        };
        for d in [&w.install, &w.user.join("mod"), &w.workshop, &w.home] {
            fs::create_dir_all(d).expect("mkdir");
        }
        w
    }

    pub(crate) fn locations(&self) -> ResolvedLocations {
        let mut l = ResolvedLocations::new();
        l.insert(KEY_INSTALL, self.install.clone());
        l.insert(KEY_USER_DIR, self.user.clone());
        l.insert(KEY_WORKSHOP, self.workshop.clone());
        l
    }

    pub(crate) fn options(&self) -> DiscoveryOptions {
        DiscoveryOptions { home: Some(self.home.clone()) }
    }

    /// `mod/<stem>.mod` with `body`.
    pub(crate) fn mod_file(&self, stem: &str, body: &str) {
        fs::write(self.user.join("mod").join(format!("{stem}.mod")), body).expect("write .mod");
    }

    /// A local mod directory plus its launcher file; returns the directory.
    pub(crate) fn local_mod(&self, stem: &str, name: &str) -> PathBuf {
        let dir = self.user.join("mod").join(stem);
        fs::create_dir_all(&dir).expect("mkdir mod");
        let body = format!("name=\"{name}\"\npath=\"{}\"\n", dir.display());
        self.mod_file(stem, &body);
        dir
    }

    /// A workshop-style mod: launcher file with a `remote_file_id` and a content dir.
    pub(crate) fn workshop_mod(&self, id: &str, name: &str) -> PathBuf {
        let dir = self.workshop.join(id);
        fs::create_dir_all(&dir).expect("mkdir ws");
        let body =
            format!("name=\"{name}\"\npath=\"{}\"\nremote_file_id=\"{id}\"\n", dir.display());
        self.mod_file(&format!("ugc_{id}"), &body);
        dir
    }

    pub(crate) fn write_user(&self, file: &str, bytes: &[u8]) -> PathBuf {
        let p = self.user.join(file);
        fs::write(&p, bytes).expect("write user file");
        p
    }
}
