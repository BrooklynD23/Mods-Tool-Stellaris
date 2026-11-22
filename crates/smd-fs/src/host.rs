//! The machine as location discovery sees it: OS, environment variables, registry.
//!
//! Everything is behind [`HostEnv`] so discovery can be tested with an injected machine
//! ([`MapEnv`]) instead of the real globals. No game knowledge lives here: callers pass
//! the names they want looked up.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::registry::{RegValue, expand_percent_vars, query};

/// The host operating system, as far as discovery cares.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Os {
    /// Windows.
    Windows,
    /// Linux.
    Linux,
    /// Anything else (macOS is deferred).
    Other,
}

impl Os {
    /// The OS this binary was built for.
    #[must_use]
    pub fn current() -> Os {
        if cfg!(windows) {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::Other
        }
    }

    /// The name profile data keys use (`windows`, `linux`, `macos`).
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Os::Windows => "windows",
            Os::Linux => "linux",
            Os::Other => "macos",
        }
    }
}

/// Why a location is a candidate: the precedence ladder, highest first.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Origin {
    /// An explicit command-line flag.
    Flag,
    /// An `SMD_*` environment variable.
    Env,
    /// Found through a Steam library.
    Steam,
    /// Found through the Windows registry.
    Registry,
    /// The OS default location.
    Default,
}

impl Origin {
    /// The lowercase name used in output.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Flag => "flag",
            Origin::Env => "env",
            Origin::Steam => "steam",
            Origin::Registry => "registry",
            Origin::Default => "default",
        }
    }
}

/// What discovery reads from the machine.
pub trait HostEnv {
    /// The OS to resolve for.
    fn os(&self) -> Os;
    /// An environment variable; `None` when unset or empty.
    fn var(&self, name: &str) -> Option<String>;
    /// Registry values under `key` (Windows only; empty elsewhere), optionally one
    /// `value` name and optionally the whole subtree.
    fn registry(&self, key: &str, value: Option<&str>, recursive: bool) -> Vec<RegValue>;
    /// The roots of the fixed and removable drives that exist (`C:\`, `D:\`...); empty
    /// off Windows.
    fn drive_roots(&self) -> DriveRoots;
}

/// The result of probing drive letters.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct DriveRoots {
    /// Drives that answered as directories in time.
    pub roots: Vec<PathBuf>,
    /// Drives that did not answer within [`DRIVE_PROBE_TIMEOUT`] (offline mapped drives);
    /// skipped, not waited for.
    pub slow: Vec<PathBuf>,
}

/// How long all drive probes together may take.
pub const DRIVE_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// `is_dir` on each root, each on its own thread, abandoning any that is still blocked
/// after `timeout` (a dead network mapping can block a probe for ~20 s).
fn probe_roots(roots: Vec<PathBuf>, timeout: Duration) -> DriveRoots {
    let (tx, rx) = mpsc::channel();
    for (n, r) in roots.iter().cloned().enumerate() {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send((n, r.is_dir()));
        });
    }
    drop(tx);
    let mut answered = vec![None; roots.len()];
    let deadline = Instant::now() + timeout;
    while let Ok((n, ok)) = rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        answered[n] = Some(ok);
    }
    let mut out = DriveRoots::default();
    for (r, a) in roots.into_iter().zip(answered) {
        match a {
            Some(true) => out.roots.push(r),
            Some(false) => {}
            None => out.slow.push(r),
        }
    }
    out
}

/// The real machine: `std::env` and the Windows registry.
#[derive(Copy, Clone, Debug, Default)]
pub struct SystemEnv;

impl HostEnv for SystemEnv {
    fn os(&self) -> Os {
        Os::current()
    }

    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.is_empty())
    }

    fn registry(&self, key: &str, value: Option<&str>, recursive: bool) -> Vec<RegValue> {
        query(key, value, recursive)
    }

    fn drive_roots(&self) -> DriveRoots {
        if self.os() != Os::Windows {
            return DriveRoots::default();
        }
        let roots = ('C'..='Z').map(|d| PathBuf::from(format!("{d}:\\"))).collect();
        probe_roots(roots, DRIVE_PROBE_TIMEOUT)
    }
}

/// An in-memory machine, for tests and embedders.
#[derive(Clone, Debug)]
pub struct MapEnv {
    /// The OS to report.
    pub os: Os,
    /// Environment variables.
    pub vars: BTreeMap<String, String>,
    /// Registry values.
    pub registry: Vec<RegValue>,
    /// Drive roots.
    pub drives: Vec<PathBuf>,
}

impl MapEnv {
    /// An empty machine of the given OS.
    #[must_use]
    pub fn new(os: Os) -> MapEnv {
        MapEnv { os, vars: BTreeMap::new(), registry: Vec::new(), drives: Vec::new() }
    }
}

impl HostEnv for MapEnv {
    fn os(&self) -> Os {
        self.os
    }

    fn var(&self, name: &str) -> Option<String> {
        self.vars.get(name).filter(|v| !v.is_empty()).cloned()
    }

    fn registry(&self, key: &str, value: Option<&str>, recursive: bool) -> Vec<RegValue> {
        let under = |k: &str| {
            k.eq_ignore_ascii_case(key)
                || (recursive && k.len() > key.len() && k[..key.len()].eq_ignore_ascii_case(key))
        };
        self.registry
            .iter()
            .filter(|v| under(&v.key) && value.is_none_or(|n| v.name.eq_ignore_ascii_case(n)))
            .cloned()
            .collect()
    }

    fn drive_roots(&self) -> DriveRoots {
        DriveRoots { roots: self.drives.clone(), slow: Vec::new() }
    }
}

/// The user's home directory. Windows: `USERPROFILE`, else `HOME` (MSYS shells set `HOME`
/// to a different folder). Elsewhere: `HOME`, else `USERPROFILE`.
#[must_use]
pub fn home_dir(env: &dyn HostEnv) -> Option<PathBuf> {
    let (first, second) =
        if env.os() == Os::Windows { ("USERPROFILE", "HOME") } else { ("HOME", "USERPROFILE") };
    env.var(first).or_else(|| env.var(second)).map(PathBuf::from)
}

/// `$XDG_DATA_HOME`, else `~/.local/share`.
#[must_use]
pub fn xdg_data_home(env: &dyn HostEnv) -> Option<PathBuf> {
    env.var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home_dir(env).map(|h| h.join(".local").join("share")))
}

fn registry_value(env: &dyn HostEnv, key: &str, name: &str) -> Option<String> {
    env.registry(key, Some(name), false).into_iter().map(|v| v.data).find(|d| !d.is_empty())
}

/// The Documents folders to try, in order, and how each was found.
///
/// Windows: the registry's `User Shell Folders\Personal`, which follows OneDrive
/// redirection (`Origin::Registry`), then `%USERPROFILE%\Documents` (`Origin::Default`),
/// because a redirected folder may not exist. Elsewhere: `~/Documents`.
#[must_use]
pub fn documents_dirs(env: &dyn HostEnv) -> Vec<(PathBuf, Origin)> {
    let mut out = Vec::new();
    if env.os() == Os::Windows {
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
        if let Some(raw) = registry_value(env, key, "Personal") {
            let p = PathBuf::from(expand_percent_vars(&raw, &|n| env.var(n)));
            out.push((p, Origin::Registry));
        }
    }
    out.extend(home_dir(env).map(|h| (h.join("Documents"), Origin::Default)));
    out
}

/// Every place Steam may be installed, in precedence order, existing or not.
#[must_use]
pub fn steam_roots(env: &dyn HostEnv) -> Vec<(PathBuf, Origin)> {
    let mut out: Vec<(PathBuf, Origin)> =
        env.var("SMD_STEAM_DIR").map(|d| (PathBuf::from(d), Origin::Env)).into_iter().collect();
    match env.os() {
        Os::Windows => {
            let reg = [
                (r"HKCU\Software\Valve\Steam", "SteamPath"),
                (r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
            ];
            for (key, name) in reg {
                let found = registry_value(env, key, name);
                out.extend(found.map(|p| (PathBuf::from(p), Origin::Registry)));
            }
            let pf = env.var("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Steam"));
            out.extend(pf.map(|p| (p, Origin::Default)));
        }
        _ => {
            let home = home_dir(env);
            let join =
                |a: &[&str]| home.as_ref().map(|h| a.iter().fold(h.clone(), |p, s| p.join(s)));
            let defaults = [join(&[".steam", "steam"]), join(&[".local", "share", "Steam"])];
            out.extend(defaults.into_iter().flatten().map(|p| (p, Origin::Default)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reg(key: &str, name: &str, data: &str) -> RegValue {
        RegValue { key: key.into(), name: name.into(), data: data.into() }
    }

    #[test]
    fn documents_follows_a_redirected_registry_value() {
        let mut e = MapEnv::new(Os::Windows);
        e.vars.insert("USERPROFILE".into(), r"C:\Users\a".into());
        let k = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders";
        e.registry.push(reg(k, "Personal", r"%USERPROFILE%\OneDrive\Documents"));
        let got = documents_dirs(&e);
        assert_eq!(got[0], (PathBuf::from(r"C:\Users\a\OneDrive\Documents"), Origin::Registry));
        assert_eq!(got[1], (PathBuf::from(r"C:\Users\a/Documents"), Origin::Default));
    }

    #[test]
    fn documents_falls_back_to_the_profile_folder() {
        let mut e = MapEnv::new(Os::Windows);
        e.vars.insert("USERPROFILE".into(), "/u".into());
        assert_eq!(documents_dirs(&e), [(PathBuf::from("/u/Documents"), Origin::Default)]);
        assert!(documents_dirs(&MapEnv::new(Os::Windows)).is_empty());
    }

    #[test]
    fn windows_home_prefers_userprofile_over_msys_home() {
        let mut e = MapEnv::new(Os::Windows);
        e.vars.insert("HOME".into(), "/msys".into());
        e.vars.insert("USERPROFILE".into(), "/u".into());
        assert_eq!(home_dir(&e), Some(PathBuf::from("/u")));
        e.os = Os::Linux;
        assert_eq!(home_dir(&e), Some(PathBuf::from("/msys")));
    }

    #[test]
    fn probes_answer_or_are_abandoned() {
        let r = probe_roots(vec![PathBuf::from("/")], Duration::from_secs(5));
        assert_eq!(r.roots, [PathBuf::from("/")]);
        assert!(r.slow.is_empty());
        let none = probe_roots(vec![PathBuf::from("/")], Duration::ZERO);
        assert_eq!(none.roots.len() + none.slow.len(), 1);
    }

    #[test]
    fn xdg_data_home_prefers_the_variable() {
        let mut e = MapEnv::new(Os::Linux);
        e.vars.insert("HOME".into(), "/h".into());
        assert_eq!(xdg_data_home(&e), Some(PathBuf::from("/h/.local/share")));
        e.vars.insert("XDG_DATA_HOME".into(), "/x".into());
        assert_eq!(xdg_data_home(&e), Some(PathBuf::from("/x")));
    }

    #[test]
    fn steam_roots_are_in_precedence_order() {
        let mut e = MapEnv::new(Os::Windows);
        e.vars.insert("SMD_STEAM_DIR".into(), "/s".into());
        e.vars.insert("ProgramFiles(x86)".into(), "/pf".into());
        e.registry.push(reg(r"HKCU\Software\Valve\Steam", "SteamPath", "/reg1"));
        e.registry.push(reg(r"HKLM\SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath", "/reg2"));
        let got: Vec<_> = steam_roots(&e).into_iter().map(|(p, o)| (p, o.as_str())).collect();
        let want =
            [("/s", "env"), ("/reg1", "registry"), ("/reg2", "registry"), ("/pf/Steam", "default")];
        let want: Vec<_> = want.iter().map(|(p, o)| (PathBuf::from(p), *o)).collect();
        assert_eq!(got, want);
        let mut l = MapEnv::new(Os::Linux);
        l.vars.insert("HOME".into(), "/h".into());
        let got: Vec<_> = steam_roots(&l).into_iter().map(|(p, _)| p).collect();
        assert_eq!(got, [PathBuf::from("/h/.steam/steam"), PathBuf::from("/h/.local/share/Steam")]);
    }

    #[test]
    fn map_env_registry_matches_subtrees_case_insensitively() {
        let mut e = MapEnv::new(Os::Windows);
        e.registry.push(reg(r"HKLM\U\One", "DisplayName", "X"));
        assert_eq!(e.registry(r"hklm\u", Some("displayname"), true).len(), 1);
        assert!(e.registry(r"HKLM\U", Some("DisplayName"), false).is_empty());
    }
}
