mod fixture;

use smd_core::{
    Activation, ActivationCertainty, Completeness, Confidence, Role, SemanticCertainty,
};

use std::path::PathBuf;

use crate::{ScanError, scan};
use fixture::World;

const SHARED: &str = "common/traits/00_shared.txt";

fn three() -> World {
    let w = World::new();
    for (id, body) in [("111", "a"), ("222", "bb"), ("333", "ccc")] {
        w.mod_with(
            id,
            &format!("Mod {id}"),
            &[(SHARED, body), (&format!("common/traits/{id}.txt"), "x")],
        );
    }
    w
}

#[test]
fn h21_directory_scan_yields_no_certain_finding() {
    let w = three();
    let out = scan(&w.request()).expect("scans");
    let p = &out.provenance;
    assert_eq!(
        (p.order_source, p.order, p.activation),
        ("directory_scan", Confidence::Fallback, Confidence::Fallback)
    );
    assert_eq!(out.findings.len(), 1, "{:#?}", out.findings);
    for f in &out.findings {
        assert_eq!(f.certainty.activation, ActivationCertainty::Conditional);
        assert_eq!(f.certainty.order, Confidence::Fallback);
        assert!(
            f.participants.iter().all(|x| x.role != Role::Winner),
            "no winner without an order"
        );
    }
}

#[test]
fn h21_inactive_sources_are_not_participants() {
    let w = three();
    w.dlc_load(&["111", "333"]);
    let out = scan(&w.request()).expect("scans");
    let inactive: Vec<&str> = out
        .inventory
        .iter()
        .filter(|m| m.activation == Activation::Inactive)
        .map(|m| m.mod_key.as_str())
        .collect();
    assert_eq!(inactive, ["steam:222"]);
    assert_eq!(out.findings.len(), 1);
    let keys: Vec<&str> = out.findings[0].participants.iter().map(|p| p.mod_key.as_str()).collect();
    assert_eq!(keys, ["steam:111", "steam:333"]);
}

#[test]
fn h12_certainty_is_computed_from_provenance_and_gaps() {
    let w = three();
    w.dlc_load(&["111", "222", "333"]);
    let out = scan(&w.request()).expect("scans");
    let f = &out.findings[0];
    let winner = f.participants.iter().find(|p| p.role == Role::Winner).expect("a stated winner");
    assert_eq!(winner.mod_key.as_str(), "steam:333", "the last in dlc_load.json wins the file");
    assert_eq!(f.certainty.activation, ActivationCertainty::Inferred);
    assert_eq!(f.certainty.order, Confidence::Inferred);
    assert_eq!(f.certainty.completeness, Completeness::Complete);
    assert_eq!(f.certainty.semantics, SemanticCertainty::Assumed, "nothing is Verified in M1");
}

#[test]
fn h9_vanilla_from_installed_tree() {
    let w = World::new();
    w.vanilla("common/defines/00_defines.txt", "v");
    w.mod_with(
        "111",
        "Mod 111",
        &[("common/defines/00_defines.txt", "m"), ("common/traits/00_traits.txt", "m")],
    );
    w.dlc_load(&["111"]);
    let out = scan(&w.request()).expect("scans");
    let subjects: Vec<&str> = out.findings.iter().map(|f| &*f.subject_text).collect();
    assert_eq!(subjects, ["common/defines/00_defines.txt"], "{subjects:?}");
    let keys: Vec<&str> = out.findings[0].participants.iter().map(|p| p.mod_key.as_str()).collect();
    assert!(keys.contains(&"vanilla"), "{keys:?}");
}

#[test]
fn benign_files_are_not_conflicts_but_are_counted() {
    let w = three();
    let out = scan(&w.request()).expect("scans");
    assert!(out.findings.iter().all(|f| &*f.subject_text != "descriptor.mod"));
    assert!(out.completeness.files_total >= 9, "{}", out.completeness.files_total);
}

#[test]
fn root_level_files_in_two_mods_are_not_conflicts() {
    let w = World::new();
    for id in ["111", "222"] {
        w.mod_with(id, &format!("Mod {id}"), &[("bigpicture.jpg", id)]);
    }
    let out = scan(&w.request()).expect("scans");
    assert!(out.findings.is_empty(), "{:#?}", out.findings);
}

#[test]
fn same_path_under_a_subdirectory_in_two_mods_still_conflicts() {
    let w = World::new();
    for id in ["111", "222"] {
        w.mod_with(id, &format!("Mod {id}"), &[("common/defines/x.txt", id)]);
    }
    let out = scan(&w.request()).expect("scans");
    let subjects: Vec<&str> = out.findings.iter().map(|f| &*f.subject_text).collect();
    assert_eq!(subjects, ["common/defines/x.txt"], "{subjects:?}");
}

#[test]
fn a_cancelled_scan_is_an_error_not_a_partial_result() {
    let w = three();
    let req = w.request();
    req.cancel.cancel();
    assert_eq!(scan(&req).unwrap_err(), ScanError::Cancelled);
}

#[test]
fn unreadable_roots_are_errors() {
    let w = three();
    let mut req = w.request();
    req.game_dir = w.game.join("nowhere");
    assert!(matches!(scan(&req), Err(ScanError::RootUnreadable { role: "game", .. })));
    let mut req = w.request();
    req.workshop_dirs.push(w.workshop.join("nowhere"));
    assert!(matches!(scan(&req), Err(ScanError::RootUnreadable { role: "workshop", .. })));
}

#[test]
fn several_workshop_roots_are_scanned_and_a_repeated_root_is_read_once() {
    let w = three();
    let second = w.home.join("lib2-workshop");
    let dir = second.join("444");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("descriptor.mod"), "name=\"Mod 444\"\nremote_file_id=\"444\"\n")
        .unwrap();
    std::fs::create_dir_all(dir.join("common/traits")).unwrap();
    std::fs::write(dir.join(SHARED), "dddd").unwrap();
    let mut req = w.request();
    req.workshop_dirs.push(PathBuf::from(format!("{}/./", w.workshop.display())));
    req.workshop_dirs.push(second);
    let out = scan(&req).expect("scans");
    let mut keys: Vec<&str> = out.inventory.iter().map(|m| m.mod_key.as_str()).collect();
    keys.sort_unstable();
    keys.retain(|k| k.starts_with("steam:"));
    assert_eq!(keys, ["steam:111", "steam:222", "steam:333", "steam:444"]);
    assert_eq!(out.findings.len(), 1);
    assert_eq!(out.findings[0].participants.len(), 4);
}

#[test]
fn scans_are_deterministic() {
    let w = three();
    w.dlc_load(&["333", "111"]);
    assert_eq!(scan(&w.request()).unwrap(), scan(&w.request()).unwrap());
}

fn is_complete(c: &smd_core::ScanCompleteness) -> bool {
    c.partial == 0 && c.unparsed.iter().all(|(_, n)| *n == 0) && c.content_incomplete.is_empty()
}

#[test]
fn dot_files_in_two_mods_are_not_conflicts() {
    let w = World::new();
    for id in ["111", "222"] {
        let files = [(".gitattributes", id), (".idea/x", id), (".idea/.gitignore", id)];
        w.mod_with(id, &format!("Mod {id}"), &files);
    }
    let out = scan(&w.request()).expect("scans");
    assert!(out.findings.is_empty(), "{:#?}", out.findings);
}

#[test]
fn an_uncontended_oversize_file_is_never_read_and_costs_nothing() {
    let w = World::new();
    w.vanilla("dlc/dlc001_x/dlc001.zip", "0123456789");
    w.vanilla("common/defines/00_big.txt", "01234567890123456789");
    let mut req = w.request();
    req.limits.file_cap = 4;
    let out = scan(&req).expect("scans");
    assert!(is_complete(&out.completeness), "{:?}", out.completeness);
    assert!(!out.diagnostics.iter().any(|d| d.code.starts_with("fs.limit")), "no one compares it");
}

#[test]
fn a_contended_oversize_vanilla_asset_archive_keeps_vanilla_complete_but_is_reported() {
    let w = World::new();
    w.vanilla("dlc/dlc001_x/dlc001.zip", "0123456789");
    w.mod_with("111", "Mod 111", &[("dlc/dlc001_x/dlc001.zip", "x")]);
    let mut req = w.request();
    req.limits.file_cap = 4;
    let out = scan(&req).expect("scans");
    let d = out
        .diagnostics
        .iter()
        .find(|d| d.code == "scan.asset_archive_unread")
        .expect("the archive is reported as not read");
    assert!(d.location.contains("dlc001.zip"), "{d:?}");
    assert!(!out.diagnostics.iter().any(|d| d.code == "fs.limit_exceeded"));
    assert!(is_complete(&out.completeness), "{:?}", out.completeness);
}

#[test]
fn a_contended_oversize_file_still_makes_the_scan_incomplete() {
    let w = World::new();
    w.vanilla("common/defines/00_big.txt", "01234567890123456789");
    w.mod_with("111", "Mod 111", &[("common/defines/00_big.txt", "01234567890123456789")]);
    let mut req = w.request();
    req.limits.file_cap = 16;
    let out = scan(&req).expect("scans");
    assert!(!is_complete(&out.completeness));
    let n: u32 = out.completeness.unparsed.iter().map(|(_, n)| *n).sum();
    assert_eq!(n, 2, "both copies of the shared oversize script count");
}

#[test]
fn progress_fires_once_per_walked_source_in_order() {
    use std::sync::{Arc, Mutex};
    let w = World::new();
    w.vanilla("common/defines/00_a.txt", "a = 1");
    w.mod_with("111", "Mod 111", &[("common/x.txt", "x = 1")]);
    w.mod_with("222", "Mod 222", &[("common/y.txt", "y = 1")]);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&seen);
    let mut req = w.request();
    req.progress = Some(crate::ProgressFn(Arc::new(move |p| sink.lock().unwrap().push(p))));
    let with = scan(&req).expect("scans");
    let seen = seen.lock().unwrap();
    assert!(seen.len() >= 3, "{seen:?}");
    for (i, p) in seen.iter().enumerate() {
        assert_eq!((p.index, p.total), (i + 1, seen.len()), "{seen:?}");
    }
    req.progress = None;
    assert_eq!(with.diagnostics, scan(&req).expect("scans").diagnostics);
}

#[test]
#[cfg(unix)] // a link in `mod/` is how discovery is made incomplete here
fn incomplete_discovery_is_a_flag_not_a_mod_and_lowers_every_finding() {
    let w = three();
    w.dlc_load(&["111", "222", "333"]);
    let clean = scan(&w.request()).expect("scans");
    assert!(!clean.completeness.discovery_incomplete);
    assert_eq!(clean.findings[0].certainty.completeness, Completeness::Complete);

    std::os::unix::fs::symlink("/nonexistent", w.user.join("mod").join("linked.mod")).unwrap();
    let out = scan(&w.request()).expect("scans");
    let c = &out.completeness;
    assert!(c.discovery_incomplete);
    assert!(c.content_incomplete.is_empty(), "no fake mod key: {:?}", c.content_incomplete);
    assert!(!out.inventory.iter().any(|m| m.mod_key.contains("discovery")));
    assert_eq!(out.findings[0].certainty.completeness, Completeness::Partial);
}

#[test]
fn a_location_gap_from_the_caller_marks_discovery_incomplete() {
    let w = three();
    w.dlc_load(&["111", "222", "333"]);
    let mut req = w.request();
    req.locations_incomplete = true;
    let out = scan(&req).expect("scans");
    assert!(out.completeness.discovery_incomplete);
    assert_eq!(out.findings[0].certainty.completeness, Completeness::Partial);
}
