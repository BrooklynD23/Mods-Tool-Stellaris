//! Gate H26: the redaction corpus. Home and account are unique strings, so a leak anywhere
//! in any output is a plain substring hit.

mod common;

use common::*;
use smd_core::Role;
use smd_report::{DiagnosticsFormat, RedactionContext, RenderOptions, render_diagnostics};

const ACCOUNT: &str = "zq_redact_7731";
const STEAM_ID: &str = "88812345";

fn context() -> RedactionContext {
    RedactionContext {
        home_dir: format!("/home/{ACCOUNT}"),
        account_name: ACCOUNT.into(),
        steam_ids: vec![STEAM_ID.into()],
    }
}

fn corpus() -> smd_report::ReportModel {
    let home = format!("/home/{ACCOUNT}");
    let p1 = format!("{home}/.steam/steam/userdata/{STEAM_ID}/remote/x.txt");
    let p2 = format!("userdata/{STEAM_ID}/config/y.txt");
    let f = finding(
        &p1,
        vec![
            participant("281990_1001", "Alpha", &p1, Role::Winner),
            participant("281990_1002", "Beta", &p2, Role::Shadowed),
        ],
    );
    let mut inv =
        row("281990_1001", "Alpha", &format!("{home}/.local/share/Steam/userdata/{STEAM_ID}/m"));
    inv.activation_basis = format!("launcher database {home}/launcher-v2.sqlite");
    inv.version = Some(format!("v1 built in {home}/build as {ACCOUNT}"));
    inv.descriptor_diagnostics.push(format!("path=\"{home}/mods/alpha\" does not exist"));
    let mut m = model(incomplete(), vec![inv], vec![f]);
    m.header.completeness.content_incomplete[0].2 = Some(format!("{home}/link").into());
    m.diagnostics.push(diag(
        "fs.io",
        &p2,
        &format!("No such file or directory (os error 2): {home}/.steam/userdata/{STEAM_ID}/z"),
    ));
    m
}

#[test]
fn h26_redaction_corpus() {
    let m = corpus();
    let opts = RenderOptions::new(context());
    let mut outputs = render_all(&m, &opts);
    for (name, fmt) in [
        ("diag-table", DiagnosticsFormat::Table),
        ("diag-json", DiagnosticsFormat::Json),
        ("diag-markdown", DiagnosticsFormat::Markdown),
    ] {
        outputs.push((name, render_diagnostics(fmt, &m.diagnostics, &opts)));
    }
    for (name, out) in &outputs {
        assert!(!out.contains(ACCOUNT), "{name} leaks the account name:\n{out}");
        assert!(!out.contains(STEAM_ID), "{name} leaks the Steam id:\n{out}");
        assert!(out.contains("~/"), "{name} shows no home replacement:\n{out}");
    }
    for (name, out) in outputs.iter().filter(|(n, _)| !n.starts_with("diag-")) {
        assert!(out.contains("~/"), "{name} shows no home replacement:\n{out}");
        assert!(out.contains("Alpha") && out.contains("281990_1001"), "{name} drops names");
    }
}

#[test]
fn h26_opt_out_is_a_positive_control() {
    let m = corpus();
    let opts = RenderOptions::new(context()).without_redaction();
    for (name, out) in render_all(&m, &opts) {
        assert!(out.contains(ACCOUNT), "{name}: control lost the account name");
        assert!(out.contains(STEAM_ID), "{name}: control lost the Steam id");
    }
    for fmt in [DiagnosticsFormat::Table, DiagnosticsFormat::Json, DiagnosticsFormat::Markdown] {
        // Markdown backslash-escapes the underscores in the account name.
        let d = render_diagnostics(fmt, &m.diagnostics, &opts).replace('\\', "");
        assert!(d.contains(ACCOUNT) && d.contains(STEAM_ID), "{fmt:?}:\n{d}");
    }
}

#[test]
fn h26_windows_paths_redact_case_insensitively() {
    let ctx = RedactionContext {
        home_dir: "C:\\Users\\Alice".into(),
        account_name: "Alice".into(),
        steam_ids: Vec::new(),
    };
    let mut inv = row("k_1", "Alpha", "c:\\users\\alice\\Documents\\mod\\x");
    inv.activation_basis = "from C:\\USERS\\ALICE\\AppData\\launcher-v2.sqlite".into();
    let m = model(complete(), vec![inv], vec![]);
    for (name, out) in render_all(&m, &RenderOptions::new(ctx)) {
        assert!(!out.to_lowercase().contains("alice"), "{name} leaks:\n{out}");
        assert!(out.contains('~'), "{name}");
    }
}

#[test]
fn h26_redaction_is_on_by_default() {
    assert!(RenderOptions::default().redact);
    let ctx = RedactionContext::from_env();
    if ctx.home_dir.len() > 1 {
        let m = model(complete(), vec![row("k", "A", &format!("{}/mods/x", ctx.home_dir))], vec![]);
        for (name, out) in render_all(&m, &RenderOptions::default()) {
            assert!(!out.contains(&ctx.home_dir), "{name} prints the home path:\n{out}");
        }
    }
    assert!(RenderOptions::new(context()).redact);
}
