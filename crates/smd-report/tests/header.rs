//! Gates H11 and H34 (output half): the completeness header, the caps, and no severity.

mod common;

use common::*;
use smd_core::{Confidence, Role};
use smd_report::{Caps, RenderOptions, Renderer};

#[test]
fn completeness_header_with_zero_findings() {
    let mut m = model(incomplete(), vec![], vec![]);
    m.header.provenance = provenance(Confidence::Inferred, Confidence::Fallback);
    for (name, out) in render_all(&m, &RenderOptions::default()) {
        let lower = out.to_lowercase();
        assert!(!lower.contains("no conflicts"), "{name}:\n{out}");
        assert!(!lower.contains("no same-path conflicts"), "{name} claims a clean scan:\n{out}");
        for needle in [
            "120",
            "unreadable",
            "oversize",
            "inferred",
            "fallback",
            "not_applicable",
            "directory_scan",
        ] {
            assert!(lower.contains(needle), "{name} lacks {needle}:\n{out}");
        }
        if name != "json" {
            assert!(lower.contains("unreadable: 2"), "{name} lacks the unreadable count:\n{out}");
            assert!(lower.contains("oversize: 1"), "{name} lacks the oversize count:\n{out}");
        }
    }
    let json: serde_json::Value =
        serde_json::from_str(&smd_report::Json.render(&m, &RenderOptions::default()))
            .expect("json");
    let h = &json["header"];
    assert_eq!((h["files_total"].as_u64(), h["partial"].as_u64()), (Some(120), Some(0)));
    assert_eq!(h["unparsed"][0]["count"].as_u64(), Some(2));
    assert_eq!(h["unparsed"][1]["count"].as_u64(), Some(1));
    assert_eq!(h["order_certainty"], "inferred");
    assert_eq!(h["activation_certainty"], "fallback");
    assert_eq!(json["summary"], serde_json::Value::Null);
}

#[test]
fn a_complete_zero_finding_scan_says_so_with_the_header() {
    let mut m = model(complete(), vec![row("k", "Alpha", "p")], vec![]);
    m.header.provenance = provenance(Confidence::Exact, Confidence::Inferred);
    for (name, out) in render_all(&m, &RenderOptions::default()) {
        assert!(out.contains("no same-path conflicts found"), "{name}");
        assert!(out.contains("120") && out.contains("exact") && out.contains("inferred"), "{name}");
    }
}

#[test]
fn participant_cap_marker() {
    let many = finding(
        "a.txt",
        (0..40)
            .map(|i| participant(&format!("k_{i:02}"), "M", "a.txt", Role::Contributor))
            .collect(),
    );
    let m = model(complete(), vec![], vec![many]);
    let opts = RenderOptions::default().with_caps(Caps { max_participants: 10, max_findings: 99 });
    for (name, out) in render_all(&m, &opts) {
        assert!(out.contains("and 30 more"), "{name}");
        let rows = out.matches("k_").count();
        // Table and Markdown print the key once per row; JSON once per participant object.
        assert_eq!(rows, 10, "{name}: {rows} participant rows");
    }
}

#[test]
fn total_finding_cap_marker() {
    let fs = (0..5).map(|i| pair(&format!("common/f{i}.txt"))).collect();
    let m = model(complete(), vec![], fs);
    let opts = RenderOptions::default().with_caps(Caps { max_participants: 10, max_findings: 3 });
    for (name, out) in render_all(&m, &opts) {
        assert_eq!(out.matches("core.file_overwrite").count(), 3, "{name}");
        if name != "json" {
            assert!(out.contains("and 2 more"), "{name}");
        }
    }
    let json: serde_json::Value =
        serde_json::from_str(&smd_report::Json.render(&m, &opts)).expect("json");
    assert_eq!(json["findings_hidden"].as_u64(), Some(2));
}

#[test]
fn findings_render_in_the_order_given() {
    let m = model(complete(), vec![], vec![pair("z.txt"), pair("a.txt")]);
    let out = smd_report::Table.render(&m, &RenderOptions::default());
    assert!(out.find("z.txt") < out.find("a.txt"));
}

#[test]
fn no_severity_in_m1_output() {
    let m = model(complete(), vec![row("k", "A", "p")], vec![pair("a"), pair("b"), pair("c")]);
    for (name, out) in render_all(&m, &RenderOptions::default()) {
        let lower = out.to_lowercase();
        assert!(!lower.contains("severity") && !lower.contains("score"), "{name}:\n{out}");
    }
}

#[test]
fn a_scan_that_read_nothing_is_not_called_clean() {
    let m = model(smd_core::ScanCompleteness::default(), vec![], vec![]);
    for (name, out) in render_all(&m, &RenderOptions::default()) {
        assert!(!out.contains("no same-path conflicts"), "{name}:\n{out}");
        assert!(out.contains("nothing was scanned"), "{name}");
    }
}

#[test]
fn the_clean_sentence_says_how_much_was_read() {
    let m = model(complete(), vec![], vec![]);
    for (name, out) in render_all(&m, &RenderOptions::default()) {
        assert!(out.contains("no same-path conflicts found among the 120 files read"), "{name}");
    }
}
