// xtask:allow(game-name) reason: the gate's own fixtures. Testing that `stellaris` is
// flagged and `stack30` is not requires the literals; a gate with no negative fixture is
// a gate nobody can trust. Recorded per-file in xtask/allowances.toml.

//! Fixtures for the `game-name` gate.

use super::*;

#[test]
fn word_boundaries_keep_the_gate_from_crying_wolf() {
    assert!(contains_word("fn load_ck3_profile", "ck3"));
    assert!(contains_word("the stellaris profile", "stellaris"));
    assert!(!contains_word("let stack30 = 1;", "ck3"));
    assert!(!contains_word("imperators", "imperator"));
    assert!(contains_word("\"imperator\"", "imperator"));
}

#[test]
fn crate_of_reads_the_workspace_layout() {
    assert_eq!(crate_of("crates/smd-rules/src/lib.rs"), Some("smd-rules"));
    assert_eq!(crate_of("xtask/src/main.rs"), Some("xtask"));
}

#[test]
fn a_hit_names_the_line_and_the_replacement() {
    let v = scan("crates/smd-rules/src/r.rs", "// for ck3\n", &["ck3".to_owned()]);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].at, "crates/smd-rules/src/r.rs:1");
    assert!(v[0].detail.contains("FeatureTag"));
}
