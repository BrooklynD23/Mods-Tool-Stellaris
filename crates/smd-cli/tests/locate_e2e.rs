//! `smd locate`, and `smd scan` with no path flags, over a fake machine in a tempdir.
//!
//! The machine is injected through the environment the binary reads (`HOME`,
//! `XDG_DATA_HOME`, `SMD_STEAM_DIR`). Nothing here names a game: the folder names the
//! profile expects are read back from `smd locate`'s own candidate list, then created.

// The Documents-style tests are unix-only (Windows reads the real registry), so their helpers
// are unused there.
#![cfg_attr(not(unix), allow(dead_code))]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{World, file, put};
use serde_json::Value;

fn smd_in(env: &[(&str, &Path)], args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_smd"));
    cmd.args(args).env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output().expect("run smd")
}

fn json_of(o: &Output) -> Value {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    serde_json::from_slice(&o.stdout).expect("smd emits JSON")
}

/// The path of the first candidate with `key` under `under`.
fn candidate(j: &Value, key: &str, under: &Path) -> PathBuf {
    let c = j["candidates"].as_array().expect("candidates").iter();
    let hit = c
        .filter(|c| c["key"] == key)
        .find(|c| Path::new(c["path"].as_str().unwrap()).starts_with(under));
    PathBuf::from(
        hit.unwrap_or_else(|| panic!("no {key} candidate under {under:?}: {j:#}"))["path"]
            .as_str()
            .unwrap(),
    )
}

struct Machine {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl Machine {
    fn path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }

    fn env(&self) -> Vec<(&'static str, PathBuf)> {
        vec![
            ("HOME", self.path("home")),
            ("XDG_DATA_HOME", self.path("xdg")),
            ("SMD_STEAM_DIR", self.path("steam")),
        ]
    }

    fn run(&self, args: &[&str]) -> Output {
        let env = self.env();
        let pairs: Vec<(&str, &Path)> = env.iter().map(|(k, v)| (*k, v.as_path())).collect();
        smd_in(&pairs, args)
    }
}

/// A Steam root whose index names libraries A and B; nothing else exists yet.
fn machine() -> Machine {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical");
    for d in ["home", "xdg", "steam/config", "libA", "libB"] {
        fs::create_dir_all(root.join(d)).expect("mkdir");
    }
    let vdf = format!(
        "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} }}",
        root.join("libA").display(),
        root.join("libB").display()
    );
    fs::write(root.join("steam/config/libraryfolders.vdf"), vdf).expect("vdf");
    Machine { _tmp: tmp, root }
}

/// Create the game (in library A), the workshop (in library B) and the user directory the
/// profile expects, with one local mod and one workshop mod. Returns their paths.
fn install(m: &Machine) -> (PathBuf, PathBuf, PathBuf) {
    let first = json_of(&m.run(&["locate", "--format", "json", "--no-redact"]));
    let game = candidate(&first, "game_dir", &m.path("libA"));
    let workshop = candidate(&first, "workshop_dir", &m.path("libB"));
    let user = candidate(&first, "user_dir", &m.path("xdg"));
    put(&game, "common/defines/00_defines.txt", "vanilla");
    let local = user.join("mod/alpha");
    put(&local, "descriptor.mod", "name=\"Alpha\"\n");
    put(&local, "common/traits/a.txt", "a");
    let body = format!(
        "name=\"Alpha\"\nversion=\"1.2\"\nsupported_version=\"3.*\"\npath=\"{}\"\n",
        local.display()
    );
    put(&user, "mod/alpha.mod", &body);
    let ws_mod = workshop.join("777");
    put(&ws_mod, "descriptor.mod", "name=\"Beta\"\nremote_file_id=\"777\"\n");
    put(&ws_mod, "common/traits/b.txt", "b");
    (game, user, workshop)
}

#[test]
#[cfg(unix)]
fn locate_json_finds_game_workshop_user_and_every_mod() {
    let m = machine();
    let (game, user, workshop) = install(&m);
    let j = json_of(&m.run(&["locate", "--format", "json", "--no-redact"]));
    assert_eq!(j["schema"], 1);
    assert_eq!(j["store"], "steam");
    assert_eq!(j["game_dir"], game.to_str().unwrap());
    assert_eq!(j["user_dir"], user.to_str().unwrap());
    assert_eq!(j["workshop_dirs"], serde_json::json!([workshop.to_str().unwrap()]));
    let libs: Vec<&str> =
        j["steam_libraries"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    for want in ["steam", "libA", "libB"] {
        assert!(libs.contains(&m.path(want).to_str().unwrap()), "{want}: {libs:?}");
    }
    let origins = |key: &str| -> Vec<&str> {
        let c = j["candidates"].as_array().unwrap().iter();
        c.filter(|c| c["key"] == key).filter_map(|c| c["origin"].as_str()).collect()
    };
    assert!(origins("game_dir").iter().all(|o| *o == "steam"), "{j:#}");
    assert_eq!(origins("user_dir")[0], "default");
    assert_eq!(origins("steam_root"), ["env", "default", "default"]);
    let names: Vec<&str> =
        j["mods"].as_array().unwrap().iter().filter_map(|m| m["name"].as_str()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    let alpha = j["mods"].as_array().unwrap().iter().find(|m| m["name"] == "Alpha").unwrap();
    assert_eq!(alpha["role"], "local");
    assert_eq!(alpha["version"], "1.2");
    assert_eq!(alpha["supported_version"], "3.*");
    assert_eq!(alpha["mod_file"], "alpha.mod");
    assert_eq!(alpha["readable"], true);
    let beta = j["mods"].as_array().unwrap().iter().find(|m| m["name"] == "Beta").unwrap();
    assert_eq!(beta["role"], "workshop");
    assert_eq!(beta["remote_file_id"], "777");
    assert!(beta["mod_file"].is_null() && beta["version"].is_null());
    assert_eq!(j["incomplete"], false);
    assert_eq!(
        (alpha["origin"].as_str(), beta["origin"].as_str()),
        (Some("mod_file"), Some("workshop_dir"))
    );
    // No launcher database here: the one diagnostic says it was not consulted.
    let codes: Vec<&str> =
        j["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["code"].as_str()).collect();
    assert_eq!(codes, ["ingest.launcher_db_not_consulted"], "{j:#}");
}

#[test]
#[cfg(unix)]
fn locate_redacts_the_home_directory_unless_told_not_to() {
    let m = machine();
    install(&m);
    let home = m.path("home");
    let env = [("HOME", home.as_path()), ("XDG_DATA_HOME", &home.join("x"))];
    fs::create_dir_all(home.join("x")).unwrap();
    let out = smd_in(&env, &["locate", "--format", "json"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains(home.to_str().unwrap()), "{text}");
}

#[test]
#[cfg(unix)]
fn scan_with_no_path_flags_locates_everything() {
    let m = machine();
    install(&m);
    let out = m.run(&["scan", "--format", "json"]);
    let j = json_of(&out);
    let names: Vec<&str> = j["header"]["inventory"]
        .as_array()
        .expect("inventory")
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(names.contains(&"Alpha") && names.contains(&"Beta"), "{names:?}");
    assert!(j["header"]["files_total"].as_u64().unwrap() >= 1);
}

#[test]
#[cfg(unix)]
fn scan_that_cannot_find_a_location_exits_2_naming_what_was_tried() {
    let m = machine();
    let out = m.run(&["scan"]);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("smd.location_not_found") && err.contains("tried"), "{err}");
    assert!(err.contains("steamapps"), "the candidates are shown: {err}");
    assert!(out.stdout.is_empty());
}

#[test]
fn locate_with_every_path_given_reports_flags_and_lists_the_mods() {
    let w = World::new("home");
    w.mod_with("1001", "One", &[file("common/traits/a.txt", "a")]);
    let (g, u, ws) =
        (w.game.to_str().unwrap(), w.user.to_str().unwrap(), w.workshop.to_str().unwrap());
    let out = Command::new(env!("CARGO_BIN_EXE_smd"))
        .args(["locate", "--format", "json", "--no-redact", "--game-dir", g, "--user-dir", u])
        .args(["--workshop-dir", ws, "--workshop-dir", ws])
        .env("HOME", &w.home)
        .output()
        .expect("run smd");
    let j = json_of(&out);
    assert_eq!(j["game_dir"], g);
    assert_eq!(j["workshop_dirs"].as_array().unwrap().len(), 1, "a repeated root is read once");
    assert_eq!(j["mods"].as_array().unwrap().len(), 1);
    assert_eq!(j["mods"][0]["remote_file_id"], "1001");
    let flags =
        j["candidates"].as_array().unwrap().iter().filter(|c| c["origin"] == "flag").count();
    assert_eq!(flags, 4);
    for fmt in ["table", "md"] {
        let o = Command::new(env!("CARGO_BIN_EXE_smd"))
            .args([
                "locate",
                "--format",
                fmt,
                "--game-dir",
                g,
                "--user-dir",
                u,
                "--workshop-dir",
                ws,
            ])
            .env("HOME", &w.home)
            .output()
            .expect("run smd");
        assert!(o.status.success());
        assert!(String::from_utf8_lossy(&o.stdout).contains("One"));
    }
}
