//! Output safety and privacy, end to end.

mod common;

use common::{World, file, stdout};

const HOME_MARK: &str = "zq_e2e_home_5150";
const OSC8_NAME: &str = "Evil \u{1b}]8;;http://example.invalid\u{7}click\u{1b}]8;;\u{7} mod";

fn world() -> World {
    let w = World::new(HOME_MARK);
    w.mod_with("1001", OSC8_NAME, &[file("common/traits/c0.txt", "one")]);
    w.mod_with("1002", "Plain", &[file("common/traits/c0.txt", "two")]);
    w
}

#[test]
fn scan_e2e_hostile_and_redacted() {
    let w = world();
    let table = stdout(&w.smd(&["--format", "table"]));
    assert!(!table.as_bytes().contains(&0x1b), "no escape byte reaches the terminal:\n{table:?}");
    for fmt in ["table", "json", "md"] {
        let out = stdout(&w.smd(&["--format", fmt]));
        assert!(!out.contains(HOME_MARK), "{fmt} leaks the home directory:\n{out}");
        assert!(out.contains("Plain"), "{fmt} still prints mod names");
        let raw = stdout(&w.smd(&["--format", fmt, "--no-redact"]));
        assert!(raw.contains(HOME_MARK), "{fmt} --no-redact shows the path:\n{raw}");
    }
}

#[test]
fn scan_e2e_error_text_is_redacted_and_escaped() {
    let w = World::new(HOME_MARK);
    let bad = w.home.join("nowhere\u{1b}[31mx");
    let run = |extra: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_smd"))
            .args(["scan", "--game-dir"])
            .arg(&bad)
            .arg("--user-dir")
            .arg(&w.user)
            .args(extra)
            .env("HOME", &w.home)
            .env_remove("USERPROFILE")
            .output()
            .expect("run smd")
    };
    let out = run(&[]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(err.contains("cannot read the game directory"), "{err:?}");
    assert!(!err.contains(HOME_MARK), "stderr leaks the home directory: {err:?}");
    assert!(!out.stderr.contains(&0x1b), "no escape byte reaches the terminal: {err:?}");
    let raw = String::from_utf8_lossy(&run(&["--no-redact"]).stderr).into_owned();
    assert!(raw.contains(HOME_MARK), "--no-redact shows the path: {raw:?}");
}
