//! End to end: the built `smd` binary over synthetic installs.

mod common;

use common::{World, file, seeded, stdout};
use serde_json::Value;

fn findings(j: &Value) -> Vec<&Value> {
    j["findings"].as_array().expect("findings array").iter().collect()
}

fn participants<'a>(f: &'a Value, role: &str) -> Vec<&'a str> {
    f["participants"]
        .as_array()
        .expect("participants")
        .iter()
        .filter(|p| p["role"] == role)
        .map(|p| p["mod_key"].as_str().expect("mod_key"))
        .collect()
}

#[test]
fn scan_e2e_k_seeded_collisions() {
    let w = World::new("home");
    let expected = seeded(&w);
    w.dlc_load(&["1001", "1002", "1003", "1004", "1005"]);
    let j = w.json();
    let all = findings(&j);
    assert_eq!(all.len(), 5, "{j:#}");
    for (path, winner) in &expected {
        let f =
            all.iter().find(|f| f["subject"] == path.as_str()).unwrap_or_else(|| panic!("{path}"));
        assert_eq!(f["rule"], "core.file_overwrite");
        assert_eq!(participants(f, "winner"), [*winner], "{path}");
    }
    assert!(all.iter().all(|f| f["subject"] != "common/traits/zz_disabled.txt"));
    assert_eq!(j["header"]["order_certainty"], "inferred");
    assert_eq!(j["header"]["activation_certainty"], "inferred");
    // Vanilla's one file, plus five enabled mods' files (descriptor, five unique files and
    // their seeded files); the disabled mod is inventoried but not walked.
    assert_eq!(j["header"]["files_total"].as_u64(), Some(1 + 5 * 6 + 11 + 1));
}

#[test]
fn scan_e2e_directory_scan_no_winner() {
    let w = World::new("home");
    seeded(&w);
    let j = w.json();
    let all = findings(&j);
    // The five seeded collisions, plus the one the disabled mod makes once nothing is
    // known to be disabled.
    assert_eq!(all.len(), 6, "{j:#}");
    for f in &all {
        assert!(participants(f, "winner").is_empty(), "{f:#}");
        assert_eq!(f["certainty"]["activation"], "conditional");
    }
    assert_eq!(j["header"]["order_source"], "directory_scan");
    assert_eq!(j["header"]["order_certainty"], "fallback");
    assert_eq!(j["header"]["activation_certainty"], "fallback");
}

#[test]
#[cfg(unix)] // relies on POSIX permission bits
fn scan_e2e_unreadable_suppresses_clean() {
    use std::os::unix::fs::PermissionsExt;
    let w = World::new("home");
    let dir = w.mod_with("1001", "One", &[file("common/traits/c0.txt", "one")]);
    w.mod_with("1002", "Two", &[file("common/traits/c0.txt", "two")]);
    w.dlc_load(&["1001", "1002"]);
    let locked = dir.join("common/traits/c0.txt");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::File::open(&locked).is_ok() {
        eprintln!("skipped: running as a user who can read mode 000 files (root)");
        return;
    }
    let j = w.json();
    let unparsed = j["header"]["unparsed"].as_array().expect("unparsed");
    assert!(unparsed.iter().any(|u| u["reason"] == "unreadable" && u["count"] == 1), "{j:#}");
    let all = findings(&j);
    assert_eq!(all.len(), 1);
    assert!(participants(all[0], "winner").is_empty(), "{:#}", all[0]);
    assert_eq!(j["scan_complete"], false);
    for fmt in ["table", "json", "md"] {
        let out = stdout(&w.smd(&["--format", fmt])).to_lowercase();
        assert!(!out.contains("no conflicts"), "{fmt}");
        assert!(!out.contains("no same-path conflicts"), "{fmt}");
    }
}

#[test]
fn scan_e2e_zero_findings_prints_header() {
    let w = World::new("home");
    for id in ["1001", "1002"] {
        w.mod_with(id, id, &[file(&format!("common/traits/only_{id}.txt"), id)]);
    }
    for fmt in ["table", "json", "md"] {
        let out = stdout(&w.smd(&["--format", fmt]));
        let lower = out.to_lowercase();
        assert!(lower.contains("no same-path conflicts found"), "{fmt}:\n{out}");
        if fmt != "json" {
            for label in [
                "order certainty",
                "activation certainty",
                "files_total",
                "unparsed",
                "content_incomplete",
            ] {
                assert!(lower.contains(label), "{fmt} prints {label}:\n{out}");
            }
        }
        assert!(lower.contains("directory_scan"), "{fmt}:\n{out}");
    }
    let j = w.json();
    assert!(j["header"]["order_certainty"].is_string());
    assert!(j["header"]["activation_certainty"].is_string());
    assert_eq!(j["scan_complete"], true);
}

#[test]
#[cfg(unix)] // relies on POSIX permission bits
fn scan_e2e_error_diagnostic_exits_3_but_conflicts_exit_0() {
    use std::os::unix::fs::PermissionsExt;
    let w = World::new("home");
    let dir = w.mod_with("1001", "One", &[file("common/traits/c0.txt", "one")]);
    w.mod_with("1002", "Two", &[file("common/traits/c0.txt", "two")]);
    w.dlc_load(&["1001", "1002"]);
    assert_eq!(w.smd(&[]).status.code(), Some(0), "conflicts alone never fail the run");
    let locked = dir.clone();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    let readable = std::fs::read_dir(&locked).is_ok();
    let out = w.smd(&["--format", "json"]);
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    if readable {
        eprintln!("skipped: running as a user who can read mode 000 directories (root)");
        return;
    }
    assert_eq!(out.status.code(), Some(3), "an Error-class diagnostic exits 3");
    let j: Value = serde_json::from_slice(&out.stdout).expect("the report is still printed");
    assert_eq!(j["scan_complete"], false);
}
