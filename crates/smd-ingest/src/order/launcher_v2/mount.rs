//! Is the launcher's directory on a local filesystem?
//!
//! SQLite's isolation guarantee holds only if the launcher's SQLite honours the same
//! locks as ours. Across an interop boundary (WSL reading a Windows drive, a Proton
//! prefix, SMB, NFS) POSIX and Windows locks do not see each other, so path A is never
//! taken there (doc 05, "Never"). An undetectable mount is treated as not local: path B
//! verifies by byte comparison and is always safe.

// Mount-type detection is real only on Linux (doc 05); elsewhere these helpers are unused.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::Path;

/// Filesystem types whose locks the launcher's SQLite may not honour.
const REMOTE: &[&str] = &[
    "drvfs",
    "9p",
    "cifs",
    "smb3",
    "smbfs",
    "nfs",
    "nfs4",
    "afs",
    "ceph",
    "glusterfs",
    "fuse.sshfs",
    "fuse.glusterfs",
    "virtiofs",
    "vboxsf",
    "prl_fs",
    "fuse.vmhgfs-fuse",
    "lustre",
];

/// Whether `fstype` is one path A must not be used across.
pub(super) fn is_remote_fstype(fstype: &str) -> bool {
    REMOTE.contains(&fstype)
}

/// `\040` and friends: the octal escapes `mountinfo` uses for spaces and tabs.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let oct = (b[i] == b'\\' && i + 3 < b.len())
            .then(|| std::str::from_utf8(&b[i + 1..i + 4]).ok())
            .flatten()
            .and_then(|d| u8::from_str_radix(d, 8).ok());
        match oct {
            Some(v) => {
                out.push(v);
                i += 4;
            }
            None => {
                out.push(b[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The filesystem type of the mount that contains `path`, from `/proc/self/mountinfo`
/// text: the mount point with the longest prefix wins.
pub(super) fn fstype_in(mountinfo: &str, path: &Path) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for line in mountinfo.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let (Some(point), Some(dash)) = (fields.get(4), fields.iter().position(|f| *f == "-"))
        else {
            continue;
        };
        let Some(fstype) = fields.get(dash + 1) else { continue };
        let point = unescape(point);
        let depth = Path::new(&point).components().count();
        if path.starts_with(&point) && best.as_ref().is_none_or(|(d, _)| depth >= *d) {
            best = Some((depth, (*fstype).to_owned()));
        }
    }
    best.map(|(_, t)| t)
}

/// Whether `dir` is on a local filesystem as far as can be told.
#[cfg(target_os = "linux")]
pub(super) fn is_local(dir: &Path) -> bool {
    let Ok(canon) = std::fs::canonicalize(dir) else { return false };
    let Ok(text) = std::fs::read_to_string("/proc/self/mountinfo") else { return false };
    fstype_in(&text, &canon).is_some_and(|t| !is_remote_fstype(&t))
}

/// A Windows path text that is not a network path: `\\?\C:\...` and `\\.\C:\...` are local,
/// `\\server\share` and `\\?\UNC\server\share` are not. A mapped drive letter looks local
/// here; telling it apart needs `GetDriveTypeW`, which needs `unsafe`, which this crate
/// forbids, so a mapped network drive is not detected.
#[cfg(any(windows, test))]
fn windows_text_is_local(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.starts_with(r"\\?\unc\") || lower.starts_with(r"\\.\unc\") {
        return false;
    }
    !lower.starts_with(r"\\") || lower.starts_with(r"\\?\") || lower.starts_with(r"\\.\")
}

/// Whether `dir` is on a local filesystem as far as can be told: on Windows, anything but
/// a UNC path.
#[cfg(windows)]
pub(super) fn is_local(dir: &Path) -> bool {
    windows_text_is_local(&dir.to_string_lossy())
}

/// Elsewhere there is no mount-type probe, so path B is used.
#[cfg(not(any(target_os = "linux", windows)))]
pub(super) fn is_local(_dir: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &str = "\
22 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw
40 22 0:35 / /mnt/c rw,noatime - 9p C:\\134 rw,aname=drvfs
41 22 0:36 / /mnt/with\\040space rw - drvfs D: rw
42 22 0:37 / /home/u/nfs rw - nfs4 srv:/x rw
";

    #[test]
    fn the_longest_mount_prefix_decides() {
        assert_eq!(fstype_in(INFO, Path::new("/home/u/x")).as_deref(), Some("ext4"));
        assert_eq!(fstype_in(INFO, Path::new("/mnt/c/Users/me")).as_deref(), Some("9p"));
        assert_eq!(fstype_in(INFO, Path::new("/home/u/nfs/d")).as_deref(), Some("nfs4"));
        assert_eq!(fstype_in(INFO, Path::new("/mnt/with space/x")).as_deref(), Some("drvfs"));
    }

    #[test]
    fn windows_unc_forms_are_remote_however_they_are_spelled() {
        for t in [r"\\srv\share\x", r"\\?\UNC\srv\share\x", r"\\?\unc\srv\s", r"\\.\UNC\s\x"] {
            assert!(!windows_text_is_local(t), "{t}");
        }
        for t in [r"C:\Users\me", r"\\?\C:\Users\me", r"\\.\C:\x"] {
            assert!(windows_text_is_local(t), "{t}");
        }
    }

    #[test]
    fn interop_and_network_types_are_remote() {
        for t in ["drvfs", "9p", "cifs", "smb3", "nfs", "nfs4"] {
            assert!(is_remote_fstype(t), "{t}");
        }
        for t in ["ext4", "btrfs", "xfs", "tmpfs", "overlay"] {
            assert!(!is_remote_fstype(t), "{t}");
        }
    }
}
