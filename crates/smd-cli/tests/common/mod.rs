//! Fixtures for the end-to-end tests: a synthetic install built in a tempdir, and a runner
//! for the built `smd` binary. Every file body is made up; nothing here needs a game.
// Each test binary uses a different subset of these helpers.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

pub struct World {
    _tmp: TempDir,
    pub home: PathBuf,
    pub game: PathBuf,
    pub user: PathBuf,
    pub workshop: PathBuf,
}

impl World {
    /// A world whose home directory is `home_name`, with the game, user and workshop
    /// directories inside it.
    pub fn new(home_name: &str) -> World {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path().canonicalize().expect("canonical").join(home_name);
        let w = World {
            game: home.join("game"),
            user: home.join("user"),
            workshop: home.join("workshop"),
            home,
            _tmp: tmp,
        };
        for d in [&w.game, &w.user.join("mod"), &w.workshop] {
            fs::create_dir_all(d).expect("mkdir");
        }
        w
    }

    pub fn vanilla(&self, rel: &str, body: &str) {
        put(&self.game, rel, body);
    }

    /// A workshop mod: a content directory and a launcher file naming it.
    pub fn mod_with(&self, id: &str, name: &str, files: &[(String, String)]) -> PathBuf {
        let dir = self.workshop.join(id);
        fs::create_dir_all(&dir).expect("mkdir mod");
        put(&dir, "descriptor.mod", &format!("name=\"{name}\"\n"));
        for (rel, body) in files {
            put(&dir, rel, body);
        }
        let launcher =
            format!("name=\"{name}\"\npath=\"{}\"\nremote_file_id=\"{id}\"\n", dir.display());
        fs::write(self.user.join("mod").join(format!("ugc_{id}.mod")), launcher).expect("launcher");
        dir
    }

    pub fn dlc_load(&self, ids: &[&str]) {
        let items: Vec<String> = ids.iter().map(|i| format!("\"mod/ugc_{i}.mod\"")).collect();
        let body = format!("{{\"enabled_mods\": [{}]}}", items.join(", "));
        fs::write(self.user.join("dlc_load.json"), body).expect("dlc_load.json");
    }

    /// `smd scan` over this world, with `HOME` set to its home directory.
    pub fn smd(&self, extra: &[&str]) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_smd"));
        cmd.arg("scan")
            .arg("--game-dir")
            .arg(&self.game)
            .arg("--user-dir")
            .arg(&self.user)
            .arg("--workshop-dir")
            .arg(&self.workshop)
            .args(extra)
            .env("HOME", &self.home)
            .env_remove("USERPROFILE");
        cmd.output().expect("run smd")
    }

    pub fn json(&self) -> serde_json::Value {
        let out = self.smd(&["--format", "json"]);
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).expect("smd emits JSON")
    }
}

pub fn put(root: &Path, rel: &str, body: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().expect("has parent")).expect("mkdir");
    fs::write(p, body).expect("write");
}

pub fn file(rel: &str, body: &str) -> (String, String) {
    (rel.to_owned(), body.to_owned())
}

pub fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Six mods (`1001`..`1006`) with 30 unique files between them and five seeded same-path
/// collisions. Returns each collision's path with its expected winner, assuming the order
/// `1001..1005` (and `1006` disabled or absent from the order).
///
/// Mod `1006` also ships `common/traits/zz_disabled.txt`, which `1002` ships too: a
/// collision only if `1006` is enabled.
pub fn seeded(w: &World) -> Vec<(String, &'static str)> {
    let ids = ["1001", "1002", "1003", "1004", "1005", "1006"];
    let seeds: [(&str, &[&str], &str); 5] = [
        ("common/traits/c0.txt", &["1001", "1002"], "1002"),
        ("common/traits/c1.txt", &["1002", "1003"], "1003"),
        ("common/traits/c2.txt", &["1003", "1004"], "1004"),
        ("common/traits/c3.txt", &["1004", "1005"], "1005"),
        ("common/traits/c4.txt", &["1001", "1003", "1005"], "1005"),
    ];
    for id in ids {
        let mut files: Vec<(String, String)> = (0..5)
            .map(|n| file(&format!("common/buildings/u_{id}_{n}.txt"), &format!("{id}/{n}")))
            .collect();
        for (path, who, _) in seeds {
            if who.contains(&id) {
                files.push(file(path, &format!("body from {id}")));
            }
        }
        if id == "1002" || id == "1006" {
            files.push(file("common/traits/zz_disabled.txt", &format!("body from {id}")));
        }
        w.mod_with(id, &format!("Mod {id}"), &files);
    }
    w.vanilla("common/defines/00_defines.txt", "vanilla");
    seeds.iter().map(|(p, _, win)| ((*p).to_owned(), *win)).collect()
}
