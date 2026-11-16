//! Golden outputs for a fixed 3-finding, 4-source fixture, compared byte for byte.
//!
//! To regenerate after an intended change: `SMD_BLESS=1 cargo test -p smd-report --test
//! golden`, then review the diff of `tests/golden/`.

mod common;

use common::*;
use smd_core::{Activation, Confidence, Role, ScanCompleteness, SourceRole, UnparsedReason};
use smd_report::{Json, Markdown, RedactionContext, RenderOptions, Renderer, ReportModel, Table};

fn fixture() -> ReportModel {
    let mut a = row("vanilla", "Base game", "game");
    a.role = SourceRole::Vanilla;
    a.activation = Activation::Active;
    a.activation_basis = "always active".into();
    a.position = Some(0);
    a.version = None;
    let mut b = row("281990_1001", "Alpha Traits", "workshop/1001");
    b.descriptor_diagnostics.push("supported_version is not a valid pattern".into());
    let mut c = row("281990_1002", "Beta [Fix](https://e.x)", "workshop/1002");
    c.version = Some("2.0.1".into());
    let mut d = row("local_gamma", "Gamma", "mod/gamma");
    d.role = SourceRole::Local;

    let f1 = pair("common/traits/00_traits.txt");
    let mut f2 = finding(
        "localisation/english/l_english.yml",
        vec![
            participant(
                "281990_1001",
                "Alpha Traits",
                "localisation/english/l_english.yml",
                Role::Winner,
            ),
            participant(
                "281990_1002",
                "Beta [Fix](https://e.x)",
                "localisation/english/l_english.yml",
                Role::Shadowed,
            ),
            participant(
                "local_gamma",
                "Gamma",
                "localisation/english/l_english.yml",
                Role::Identical,
            ),
        ],
    );
    f2.certainty.completeness = smd_core::Completeness::Partial;
    let f3 = finding(
        "events/pirate_events.txt",
        vec![participant("local_gamma", "Gamma", "events/pirate_events.txt", Role::Contributor)],
    );
    let completeness = ScanCompleteness {
        files_total: 120,
        partial: 1,
        unparsed: [(UnparsedReason::Oversize, 1)].into_iter().collect(),
        domains: vec!["event".into()],
        content_incomplete: Vec::new(),
    };
    let mut m = model(completeness, vec![a, b, c, d], vec![f1, f2, f3]);
    m.header.provenance = provenance(Confidence::Inferred, Confidence::Fallback);
    m.diagnostics.push(diag(
        "clausewitz.unbalanced_brace",
        "events/pirate_events.txt",
        "brace never closed",
    ));
    m
}

fn check(name: &str, actual: &str) {
    let path = format!("{}/tests/golden/{name}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("SMD_BLESS").is_some() {
        std::fs::write(&path, actual).expect("write golden");
    }
    let expected = std::fs::read_to_string(&path).expect("golden file exists");
    assert_eq!(actual, expected, "{name} differs from its golden file");
}

fn opts() -> RenderOptions {
    RenderOptions::new(RedactionContext::default())
}

#[test]
fn golden_table() {
    check("report.txt", &Table.render(&fixture(), &opts()));
}

#[test]
fn golden_json() {
    check("report.json", &Json.render(&fixture(), &opts()));
}

#[test]
fn golden_markdown() {
    check("report.md", &Markdown.render(&fixture(), &opts()));
}
