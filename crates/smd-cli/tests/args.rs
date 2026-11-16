//! Command-line handling and determinism.

mod common;

use std::process::Command;

use common::{World, seeded, stdout};

fn smd(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_smd")).args(args).output().expect("run smd")
}

#[test]
fn cli_rejects_bad_args() {
    for args in [
        &["scan", "--bogus"][..],
        &["scan", "--user-dir", "/tmp"],
        &["scan", "--game-dir", "/tmp"],
        &["scan", "--game-dir"],
        &["scan", "--game-dir", "/a", "--user-dir", "/b", "--format", "xml"],
        &["scan", "--game-dir", "/a", "--user-dir", "/b", "--max-findings", "many"],
        &["frobnicate"],
        &[],
    ] {
        let out = smd(args);
        assert!(!out.status.success(), "{args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("usage: smd scan"), "{args:?}: {err}");
    }
}

#[test]
fn unreadable_roots_exit_non_zero_with_a_message() {
    let w = World::new("home");
    let out = smd(&["scan", "--game-dir", "/nonexistent-smd-game", "--user-dir"]
        .iter()
        .copied()
        .chain([w.user.to_str().expect("utf-8")])
        .collect::<Vec<_>>());
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("cannot read the game directory"));
    assert!(out.stdout.is_empty(), "no report is printed for a scan that did not run");
}

#[test]
fn help_succeeds() {
    let out = smd(&["--help"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("usage: smd scan"));
}

#[test]
fn scan_output_deterministic() {
    let w = World::new("home");
    seeded(&w);
    w.dlc_load(&["1005", "1001", "1003", "1002"]);
    for fmt in ["json", "table", "md"] {
        let a = w.smd(&["--format", fmt]);
        let b = w.smd(&["--format", fmt]);
        assert!(a.status.success());
        assert_eq!(a.stdout, b.stdout, "{fmt}");
    }
}

#[test]
fn caps_bound_the_findings_printed() {
    let w = World::new("home");
    seeded(&w);
    let out = stdout(&w.smd(&["--format", "table", "--max-findings", "2"]));
    assert!(out.contains("... and 4 more findings"), "{out}");
}
