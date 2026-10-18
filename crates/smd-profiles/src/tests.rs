use smd_core::SamePath;

use super::*;

fn shipped() -> Profile {
    Profile::shipped_default().expect("the shipped profile reads")
}

#[test]
fn profile_classifies_same_path() {
    let p = shipped();
    assert_eq!(p.classify("common/traits/00_traits.txt"), SamePath::Replace);
    assert_eq!(p.classify("localisation/english/a_l_english.yml"), SamePath::Unknown);
    assert_eq!(p.classify("localisation/replace/a_l_english.yml"), SamePath::Unknown);
    assert_eq!(p.classify("gfx/models/x.dds"), SamePath::Replace);
    assert_eq!(p.replace_path_scope(), ReplacePathScope::Unknown);
}

#[test]
fn values_come_from_the_text_not_from_the_code() {
    let text = r#"
        [load_order]
        same_filename_replaces_whole_file = false
        replace_path_scope = "vanilla_only"
        [[dir]]
        glob = "a/**"
        [[dir]]
        glob = "a/b/**"
        same_path = "replace"
    "#;
    let p = Profile::from_toml(text).unwrap();
    assert_eq!(p.replace_path_scope(), ReplacePathScope::VanillaOnly);
    assert_eq!(p.classify("a/x.txt"), SamePath::Unknown);
    assert_eq!(p.classify("a/b/x.txt"), SamePath::Replace);
    assert_eq!(p.classify("z/x.txt"), SamePath::Unknown, "under no [[dir]], default false");
    let replacing = "[load_order]\nsame_filename_replaces_whole_file = true\n[[dir]]\nglob = \"a/**\"\nsame_path = \"unknown\"\n";
    let p = Profile::from_toml(replacing).unwrap();
    assert_eq!(p.classify("z/x.txt"), SamePath::Replace, "an uncovered path follows the default");
    assert_eq!(p.classify("a/x.txt"), SamePath::Unknown, "a [[dir]] override still wins");
}

#[test]
fn modelled_keys_with_unknown_values_are_rejected_and_other_keys_ignored() {
    let bad = "[load_order]\nreplace_path_scope = \"sideways\"\n";
    assert!(matches!(Profile::from_toml(bad), Err(ProfileError::Value { .. })));
    let extra = "[load_order]\nsome_future_key = 1\n[[dir]]\nglob = \"a/**\"\nmerge = \"x\"\n";
    assert!(Profile::from_toml(extra).is_ok());
}

#[test]
fn benign_paths_and_sources_are_read() {
    let p = shipped();
    assert!(p.is_benign("descriptor.mod"));
    assert!(p.is_benign("thumbnail.png"));
    assert!(!p.is_benign("common/traits/00_traits.txt"));
    let names: Vec<String> = p.load_order_sources().iter().map(|s| s.name.to_string()).collect();
    assert_eq!(names, ["launcher_v2_sqlite", "dlc_load_json", "directory_scan"]);
}
