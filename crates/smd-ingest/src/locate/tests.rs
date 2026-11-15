//! Location discovery over an injected machine and tempdir fixtures. The profile is a
//! made-up game, so nothing here names a real one.

use std::fs;
use std::path::{Path, PathBuf};

use smd_fs::{MapEnv, Origin, Os, RegValue};
use smd_profiles::Profile;
use tempfile::TempDir;

use super::*;

const PROFILE: &str = r#"
[game]
id = "g"
steam_id = 4242
[locations]
user_dir.windows = "{DOCUMENTS}/Pub/G"
user_dir.linux = "{XDG_DATA_HOME}/Pub/G"
user_dir.proton = "{PROTON_PREFIX}/drive_c/users/u/Documents/Pub/G"
workshop = "{STEAM_LIB}/steamapps/workshop/content/4242"
install = "{STEAM_LIB}/steamapps/common/Gee"
steam_library_index = ["config/libraryfolders.vdf", "steamapps/libraryfolders.vdf"]
[stores]
executable = "g.exe"
gog_display_name_prefix = "Gee"
xbox_install = "XboxGames/Gee/Content"
extra_user_dirs = ["{DOCUMENTS}/Pub/G Alt"]
"#;

struct Fx {
    _tmp: TempDir,
    root: PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonical");
        Fx { _tmp: tmp, root }
    }

    fn dir(&self, rel: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(&p).expect("mkdir");
        p
    }

    fn file(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.root.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(&p, body).expect("write");
        p
    }

    fn linux(&self) -> MapEnv {
        let mut e = MapEnv::new(Os::Linux);
        e.vars.insert("HOME".into(), self.root.join("home").display().to_string());
        e
    }

    /// Steam under home with libraries A and B named by the new-format index.
    fn steam(&self) {
        let (a, b) = (self.dir("libA"), self.dir("libB"));
        let root = self.dir("home/.steam/steam");
        let vdf = format!(
            "\"libraryfolders\" {{ \"0\" {{ \"path\" \"{}\" }} \"1\" {{ \"path\" \"{}\" }} \"2\" {{ \"path\" \"{}\" }} }}",
            root.display(),
            a.display(),
            b.display()
        );
        self.file("home/.steam/steam/config/libraryfolders.vdf", &vdf);
    }
}

fn profile() -> Profile {
    Profile::from_toml(PROFILE).expect("profile")
}

fn paths(l: &Located, key: &str) -> Vec<(PathBuf, Origin, bool)> {
    let of = |c: &Candidate| (c.path.clone(), c.origin, c.exists);
    l.candidates.iter().filter(|c| c.key == key).map(of).collect()
}

#[test]
fn steam_game_in_one_library_workshop_in_another() {
    let fx = Fx::new();
    fx.steam();
    fx.file(
        "libA/steamapps/appmanifest_4242.acf",
        "\"AppState\" { \"installdir\" \"Gee Custom\" }",
    );
    let game = fx.dir("libA/steamapps/common/Gee Custom");
    let ws = fx.dir("libB/steamapps/workshop/content/4242");
    let user = fx.dir("home/.local/share/Pub/G");
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    assert_eq!(got.store, Store::Steam);
    assert_eq!(got.game_dir, Some(game));
    assert_eq!(got.user_dir, Some(user));
    assert_eq!(got.workshop_dirs, [ws]);
    assert_eq!(got.steam_libraries.len(), 3, "root, A and B: {:?}", got.steam_libraries);
    assert!(got.diagnostics.is_empty(), "{:?}", got.diagnostics);
    let libs = paths(&got, "steam_library");
    assert!(libs.iter().all(|(_, o, e)| *o == Origin::Steam && *e));
}

#[test]
fn every_library_with_a_workshop_dir_contributes_one() {
    let fx = Fx::new();
    fx.steam();
    let a = fx.dir("libA/steamapps/workshop/content/4242");
    let b = fx.dir("libB/steamapps/workshop/content/4242");
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    assert_eq!(got.workshop_dirs, [a, b]);
}

#[test]
fn flag_beats_env_beats_steam_and_every_candidate_is_recorded() {
    let fx = Fx::new();
    fx.steam();
    let steam_game = fx.dir("libA/steamapps/common/Gee");
    let (env_game, flag_game) = (fx.dir("envgame"), fx.dir("flaggame"));
    let mut env = fx.linux();
    env.vars.insert("SMD_GAME_DIR".into(), env_game.display().to_string());
    let over = |g: Option<PathBuf>| Overrides { game_dir: g, ..Overrides::default() };
    let got = locate(&profile(), &over(Some(flag_game.clone())), &env);
    assert_eq!((got.game_dir, got.store), (Some(flag_game), Store::Other));
    let got = locate(&profile(), &over(None), &env);
    assert_eq!(got.game_dir, Some(env_game.clone()));
    env.vars.remove("SMD_GAME_DIR");
    let got = locate(&profile(), &over(None), &env);
    assert_eq!((got.game_dir.clone(), got.store), (Some(steam_game), Store::Steam));
    assert!(paths(&got, "game_dir").iter().all(|(_, o, _)| *o == Origin::Steam));
}

#[test]
fn a_missing_flag_stops_the_search_and_a_missing_env_falls_through() {
    let fx = Fx::new();
    fx.steam();
    let steam_game = fx.dir("libA/steamapps/common/Gee");
    let mut env = fx.linux();
    env.vars.insert("SMD_GAME_DIR".into(), fx.root.join("nope").display().to_string());
    let flagged = Overrides { game_dir: Some(fx.root.join("gone")), ..Overrides::default() };
    let got = locate(&profile(), &flagged, &env);
    assert_eq!((got.game_dir, got.store), (None, Store::Unknown));
    assert!(got.diagnostics.iter().any(|d| d.code == codes::LOCATE_OVERRIDE_MISSING));
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!(got.game_dir, Some(steam_game));
    assert!(got.diagnostics.iter().any(|d| d.code == codes::LOCATE_OVERRIDE_MISSING));
    let first = got.candidates.iter().find(|c| c.key == "game_dir").expect("candidates");
    assert_eq!((first.key, first.origin, first.exists), ("game_dir", Origin::Env, false));
}

#[test]
fn nothing_found_is_unknown_with_a_diagnostic_per_missing_location() {
    let fx = Fx::new();
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    assert_eq!(got.store, Store::Unknown);
    let subjects: Vec<&str> = got.diagnostics.iter().map(|d| &*d.subject).collect();
    assert_eq!(subjects, ["game_dir", "user_dir", "workshop_dir"]);
    assert!(got.diagnostics.iter().all(|d| d.code == codes::LOCATE_UNRESOLVED));
}

#[test]
fn a_bad_index_is_a_diagnostic_and_the_root_still_counts_as_a_library() {
    let fx = Fx::new();
    fx.file("home/.steam/steam/config/libraryfolders.vdf", "\"libraryfolders\" {");
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    let bad: Vec<_> =
        got.diagnostics.iter().filter(|d| d.code == codes::LOCATE_VDF_INVALID).collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(got.steam_libraries, [fx.root.join("home/.steam/steam")]);
}

#[test]
fn a_manifest_install_dir_cannot_escape_the_library() {
    let fx = Fx::new();
    fx.steam();
    fx.file("libA/steamapps/appmanifest_4242.acf", "\"AppState\" { \"installdir\" \"../../etc\" }");
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    assert_eq!(got.game_dir, None);
    assert!(paths(&got, "game_dir").iter().all(|(p, _, _)| p.ends_with("steamapps/common/Gee")));
}

#[test]
fn steam_dir_env_var_is_the_first_root_tried() {
    let fx = Fx::new();
    let root = fx.dir("elsewhere");
    let game = fx.dir("elsewhere/steamapps/common/Gee");
    let mut env = fx.linux();
    env.vars.insert("SMD_STEAM_DIR".into(), root.display().to_string());
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!(got.game_dir, Some(game));
    let first = &paths(&got, "steam_root")[0];
    assert_eq!((&first.0, first.1), (&root, Origin::Env));
}

fn reg(key: &str, name: &str, data: &str) -> RegValue {
    RegValue { key: key.into(), name: name.into(), data: data.into() }
}

#[test]
fn documents_redirected_by_the_registry_is_found_and_labelled() {
    let fx = Fx::new();
    let user = fx.dir("OneDrive/Documents/Pub/G");
    let mut env = MapEnv::new(Os::Windows);
    env.vars.insert("USERPROFILE".into(), fx.root.join("plain").display().to_string());
    let k = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
    env.registry.push(reg(k, "Personal", &format!("{}/OneDrive/Documents", fx.root.display())));
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!(got.user_dir, Some(user.clone()));
    assert_eq!(paths(&got, "user_dir"), [(user, Origin::Registry, true)]);
}

#[test]
fn extra_user_dir_candidates_come_after_the_os_default() {
    let fx = Fx::new();
    let alt = fx.dir("plain/Documents/Pub/G Alt");
    let mut env = MapEnv::new(Os::Windows);
    env.vars.insert("USERPROFILE".into(), fx.root.join("plain").display().to_string());
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!(got.user_dir, Some(alt));
    let tried = paths(&got, "user_dir");
    assert_eq!(tried.len(), 2);
    assert!(tried[0].0.ends_with("Pub/G") && !tried[0].2);
}

#[test]
fn a_proton_prefix_inside_a_library_is_a_steam_candidate() {
    let fx = Fx::new();
    fx.steam();
    let user = fx.dir("libB/steamapps/compatdata/4242/pfx/drive_c/users/u/Documents/Pub/G");
    let got = locate(&profile(), &Overrides::default(), &fx.linux());
    assert_eq!(got.user_dir, Some(user.clone()));
    assert!(paths(&got, "user_dir").contains(&(user, Origin::Steam, true)));
}

fn gog_env(fx: &Fx, location: &Path) -> MapEnv {
    let mut env = MapEnv::new(Os::Windows);
    env.vars.insert("USERPROFILE".into(), fx.root.join("plain").display().to_string());
    let un = r"HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
    env.registry.push(reg(&format!(r"{un}\1_a"), "DisplayName", "Gee Soundtrack"));
    env.registry.push(reg(
        &format!(r"{un}\1_a"),
        "InstallLocation",
        &fx.root.join("ost").display().to_string(),
    ));
    env.registry.push(reg(&format!(r"{un}\1_b"), "DisplayName", "gee Galaxy Edition"));
    let at = location.display().to_string();
    env.registry.push(reg(&format!(r"{un}\1_b"), "InstallLocation", &at));
    env
}

#[test]
fn an_installer_entry_is_accepted_only_when_it_holds_the_executable() {
    let fx = Fx::new();
    fx.dir("ost");
    let game = fx.dir("GOG/Gee");
    let env = gog_env(&fx, &game);
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!(got.game_dir, None, "no executable yet");
    assert!(paths(&got, "game_dir").iter().all(|(_, o, e)| *o == Origin::Registry && !*e));
    fx.file("GOG/Gee/g.exe", "");
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!((got.game_dir, got.store), (Some(game), Store::Gog));
}

#[test]
fn an_xbox_install_is_found_under_a_drive_root() {
    let fx = Fx::new();
    let drive = fx.dir("D");
    let game = fx.dir("D/XboxGames/Gee/Content");
    fx.file("D/XboxGames/Gee/Content/g.exe", "");
    let mut env = MapEnv::new(Os::Windows);
    env.drives = vec![fx.root.join("C"), drive];
    let got = locate(&profile(), &Overrides::default(), &env);
    assert_eq!((got.game_dir.clone(), got.store), (Some(game), Store::Xbox));
    assert_eq!(paths(&got, "game_dir").len(), 2);
}

#[test]
fn workshop_flags_replace_the_search_and_are_deduplicated() {
    let fx = Fx::new();
    fx.steam();
    fx.dir("libA/steamapps/workshop/content/4242");
    let w = fx.dir("ws");
    let again = PathBuf::from(format!("{}/./", w.display()));
    let over = Overrides {
        workshop_dirs: vec![w.clone(), again, fx.root.join("gone")],
        ..Overrides::default()
    };
    let got = locate(&profile(), &over, &fx.linux());
    assert_eq!(got.workshop_dirs, [w]);
    assert!(got.diagnostics.iter().any(|d| d.code == codes::LOCATE_OVERRIDE_MISSING));
}

#[test]
#[cfg(unix)]
fn a_linked_directory_is_listed_once() {
    let fx = Fx::new();
    let real = fx.dir("real");
    std::os::unix::fs::symlink(&real, fx.root.join("link")).unwrap();
    let mut libs = vec![fx.root.join("link")];
    push_unique(&mut libs, real);
    assert_eq!(libs, [fx.root.join("link")]);
}
