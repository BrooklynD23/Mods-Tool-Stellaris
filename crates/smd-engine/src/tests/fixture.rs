//! A synthetic install: a game tree, a user directory, a workshop, a home. Every file body
//! is made up; nothing here reads a real game install.

use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use crate::ScanRequest;

pub(crate) struct World {
    _tmp: TempDir,
    pub game: PathBuf,
    pub user: PathBuf,
    pub workshop: PathBuf,
    pub home: PathBuf,
}

impl World {
    pub(crate) fn new() -> World {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonical");
        let w = World {
            game: root.join("game"),
            user: root.join("user"),
            workshop: root.join("workshop"),
            home: root.join("home"),
            _tmp: tmp,
        };
        for d in [&w.game, &w.user.join("mod"), &w.workshop, &w.home] {
            fs::create_dir_all(d).expect("mkdir");
        }
        w
    }

    pub(crate) fn request(&self) -> ScanRequest {
        let mut r = ScanRequest::new(self.game.clone(), self.user.clone());
        r.workshop_dirs.push(self.workshop.clone());
        r.home = Some(self.home.clone());
        r
    }

    /// A file in the installed game tree.
    pub(crate) fn vanilla(&self, rel: &str, body: &str) {
        put(&self.game, rel, body);
    }

    /// A workshop mod `id` named `name`, with a launcher file and the given files.
    pub(crate) fn mod_with(&self, id: &str, name: &str, files: &[(&str, &str)]) {
        let dir = self.workshop.join(id);
        fs::create_dir_all(&dir).expect("mkdir mod");
        put(&dir, "descriptor.mod", &format!("name=\"{name}\"\n"));
        for (rel, body) in files {
            put(&dir, rel, body);
        }
        let launcher =
            format!("name=\"{name}\"\npath=\"{}\"\nremote_file_id=\"{id}\"\n", dir.display());
        let file = self.user.join("mod").join(format!("ugc_{id}.mod"));
        fs::write(file, launcher).expect("write launcher file");
    }

    /// `dlc_load.json` enabling `ids`, in that order.
    pub(crate) fn dlc_load(&self, ids: &[&str]) {
        let items: Vec<String> = ids.iter().map(|i| format!("\"mod/ugc_{i}.mod\"")).collect();
        let body = format!("{{\"enabled_mods\": [{}]}}", items.join(", "));
        fs::write(self.user.join("dlc_load.json"), body).expect("write dlc_load.json");
    }
}

fn put(root: &std::path::Path, rel: &str, body: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().expect("has parent")).expect("mkdir");
    fs::write(p, body).expect("write");
}
