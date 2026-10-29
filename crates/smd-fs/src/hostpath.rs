//! Identity of absolute *host* directory paths.
//!
//! This is deliberately separate from [`crate::NormPath`]: a `NormPath` is a root-relative
//! key for a file inside a mod, normalised the way the game's own VFS does (NFC, `/`,
//! lowercase) and never touches the host. A host path names a directory on the machine
//! running the tool and arrives in whatever spelling the launcher, Steam's registry, or
//! the user wrote it: `D:/Steam/..`, `d:\steam\..\`, `/mnt/d/Steam/..`. Two spellings are the
//! same directory when the host file system would resolve them to the same entry, which is
//! a fact about the host, not about the game.
//!
//! The comparison is lexical (nothing is read from disk, so it works for paths of another
//! host's spelling) and symmetric. Callers resolve links and `..` first where they can.
//!
//! The rule, in [`same_host_dir`]:
//! * Separators: `\` is a separator, and `/` is too, when the path is Windows-style (a
//!   drive letter, `\\` UNC or `\\?\` prefix) or the host is Windows. On other hosts a
//!   backslash in a POSIX path is an ordinary name character and is left alone.
//! * Repeated separators, `.` segments, and trailing separators do not matter.
//! * Case is folded when the host is Windows, **or either path is Windows-style, or either
//!   is under a WSL drive mount (`/mnt/<letter>/`)**, since those file systems are
//!   case-insensitive. Otherwise the comparison is exact: ext4 and friends are
//!   case-sensitive and `Mods` and `mods` are different directories there.
//!
//! Paths that are not valid UTF-8 are compared exactly.

use std::path::Path;

/// Whether two absolute host paths name the same directory. See the module docs.
#[must_use]
pub fn same_host_dir(a: &Path, b: &Path) -> bool {
    same_host_dir_with(a, b, cfg!(windows))
}

/// [`same_host_dir`] with the host given (`true`: a Windows host), so the rule can be
/// exercised on any platform.
#[must_use]
pub fn same_host_dir_with(a: &Path, b: &Path, host_is_windows: bool) -> bool {
    let (Some(sa), Some(sb)) = (a.to_str(), b.to_str()) else {
        return a == b;
    };
    let (wa, wb) = (is_windows_style(sa), is_windows_style(sb));
    let fold = host_is_windows || wa || wb || is_wsl_drive(sa) || is_wsl_drive(sb);
    parts(sa, wa || host_is_windows, fold) == parts(sb, wb || host_is_windows, fold)
}

fn is_windows_style(s: &str) -> bool {
    let b = s.as_bytes();
    (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') || s.starts_with("\\\\")
}

/// `/mnt/<letter>` or below: a WSL mount of a Windows drive (case-insensitive by default).
fn is_wsl_drive(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("/mnt/") else { return false };
    let b = rest.as_bytes();
    !b.is_empty() && b[0].is_ascii_alphabetic() && (b.len() == 1 || b[1] == b'/')
}

/// Leading-separator count (0, 1, or 2 for UNC) and the segments.
fn parts(s: &str, backslash_is_sep: bool, fold: bool) -> (usize, Vec<String>) {
    let mut s = if backslash_is_sep { s.replace('\\', "/") } else { s.to_owned() };
    if backslash_is_sep {
        // `\\?\C:\x` and `\\?\UNC\srv\share` are the same paths without the prefix.
        if let Some(rest) = s.strip_prefix("//?/") {
            s = match rest.strip_prefix("UNC/") {
                Some(unc) => format!("//{unc}"),
                None => rest.to_owned(),
            };
        }
    }
    let lead = s.bytes().take_while(|&c| c == b'/').count();
    let lead = if lead >= 2 && backslash_is_sep { 2 } else { lead.min(1) };
    let segs = s
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .map(|p| if fold { p.to_lowercase() } else { p.to_owned() })
        .collect();
    (lead, segs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: &str, b: &str) -> bool {
        same_host_dir_with(Path::new(a), Path::new(b), false)
    }

    #[test]
    fn case_and_separators_do_not_matter_for_windows_paths() {
        let ws = "d:/steam/steamapps/workshop/content/281990/123";
        assert!(same("D:/Steam/steamapps/workshop/content/281990/123", ws));
        assert!(same(r"D:\Steam\steamapps\workshop\content\281990\123", ws));
        assert!(same(r"D:\Steam\steamapps\workshop\content\281990\123\", ws));
        assert!(same("D:/Steam//steamapps/./workshop/content/281990/123/", ws));
        assert!(same(r"\\?\D:\Steam\steamapps\workshop\content\281990\123", ws));
    }

    #[test]
    fn different_directories_are_distinct() {
        assert!(!same("D:/Steam/ws/123", "d:/steam/ws/124"));
        assert!(!same("D:/Steam/ws/123", "d:/steam/ws"));
        assert!(!same("D:/Steam/ws/123", "e:/steam/ws/123"));
        assert!(!same(r"\\srv\share\x", r"\srv\share\x"));
    }

    #[test]
    fn wsl_drive_mounts_fold_case() {
        assert!(same("/mnt/d/Steam/ws/1/", "/mnt/d/steam/ws/1"));
        assert!(!same("/mnt/d/Steam/ws/1", "/mnt/d/steam/ws/2"));
    }

    #[test]
    fn other_posix_paths_are_exact() {
        assert!(!same("/home/u/Mods/1", "/home/u/mods/1"));
        assert!(same("/home/u/mods/1/", "/home/u//mods/./1"));
        assert!(!same("/home/u/a\\b", "/home/u/a/b"), "backslash is a name char on POSIX");
        assert!(!same("/mntx/d/A", "/mntx/d/a"));
    }

    #[test]
    fn a_windows_host_folds_everything() {
        let eq = |a: &str, b: &str| same_host_dir_with(Path::new(a), Path::new(b), true);
        assert!(eq("/x/Mods/1", "/x/mods/1"));
        assert!(eq(r"\x\Mods", "/x/mods/"));
    }
}
