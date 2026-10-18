mod fixture;

use smd_core::{
    Activation, ActivationCertainty, Completeness, Confidence, Role, SemanticCertainty,
};

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
    assert_eq!(inactive, ["222"]);
    assert_eq!(out.findings.len(), 1);
    let keys: Vec<&str> = out.findings[0].participants.iter().map(|p| p.mod_key.as_str()).collect();
    assert_eq!(keys, ["111", "333"]);
}

#[test]
fn h12_certainty_is_computed_from_provenance_and_gaps() {
    let w = three();
    w.dlc_load(&["111", "222", "333"]);
    let out = scan(&w.request()).expect("scans");
    let f = &out.findings[0];
    let winner = f.participants.iter().find(|p| p.role == Role::Winner).expect("a stated winner");
    assert_eq!(winner.mod_key.as_str(), "333", "the last in dlc_load.json wins the file");
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
fn a_cancelled_scan_is_an_error_not_a_partial_result() {
    let w = three();
    let req = w.request();
    req.cancel.cancel();
    assert_eq!(scan(&req).unwrap_err(), ScanError::Cancelled);
}

#[test]
fn unreadable_roots_and_unsupported_requests_are_errors() {
    let w = three();
    let mut req = w.request();
    req.game_dir = w.game.join("nowhere");
    assert!(matches!(scan(&req), Err(ScanError::RootUnreadable { role: "game", .. })));
    let mut req = w.request();
    req.workshop_dirs.push(w.workshop.clone());
    assert!(matches!(scan(&req), Err(ScanError::Unsupported(_))));
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
fn an_oversize_vanilla_asset_archive_keeps_the_scan_complete_but_is_reported() {
    let w = World::new();
    w.vanilla("dlc/dlc001_x/dlc001.zip", "0123456789");
    let mut req = w.request();
    req.limits.file_cap = 4;
    let out = scan(&req).expect("scans");
    assert!(is_complete(&out.completeness), "{:?}", out.completeness);
    let d = out
        .diagnostics
        .iter()
        .find(|d| d.code == "scan.asset_archive_unread")
        .expect("the archive is reported as not read");
    assert!(d.location.contains("dlc001.zip"), "{d:?}");
    assert!(!out.diagnostics.iter().any(|d| d.code == "fs.limit_exceeded"));
}

#[test]
fn an_ordinary_oversize_file_still_makes_the_scan_incomplete() {
    let w = World::new();
    w.vanilla("common/defines/00_big.txt", "01234567890123456789");
    w.mod_with("111", "Mod 111", &[("dlc/dlc001_x/dlc001.zip", "01234567890123456789")]);
    let mut req = w.request();
    req.limits.file_cap = 16;
    let out = scan(&req).expect("scans");
    assert!(!is_complete(&out.completeness));
    let n: u32 = out.completeness.unparsed.iter().map(|(_, n)| *n).sum();
    assert_eq!(n, 2, "a vanilla script and a mod's archive-shaped file both count");
}
