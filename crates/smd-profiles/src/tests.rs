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
    let extra = "[load_order]\nsome_future_key = 1\n[[dir]]\nglob = \"a/**\"\nfuture = \"x\"\n";
    assert!(Profile::from_toml(extra).is_ok());
    // `merge` became a modelled key in M2: an unknown value is now rejected.
    let merge = "[[dir]]\nglob = \"a/**\"\nmerge = \"x\"\n";
    assert!(matches!(Profile::from_toml(merge), Err(ProfileError::Value { .. })));
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

#[test]
fn dot_files_and_dot_directories_are_benign() {
    let p = shipped();
    for path in [".gitattributes", ".idea/.gitignore", ".idea/x", "gfx/.vscode/settings.json"] {
        assert!(p.is_benign(path), "{path}");
    }
    assert!(!p.is_benign("common/traits/a.txt"));
}

#[test]
fn root_level_files_are_benign_and_subdirectory_files_are_not() {
    let p = shipped();
    for path in ["bigpicture.jpg", "thumbnail.png"] {
        assert!(p.is_benign(path), "{path}");
    }
    for path in ["common/traits/x.txt", "gfx/a.dds"] {
        assert!(!p.is_benign(path), "{path}");
    }
}

#[test]
fn asset_archives_are_declared_by_pattern() {
    let p = shipped();
    assert!(p.is_asset_archive("dlc/dlc001_symbols_of_domination/dlc001.zip"));
    assert!(!p.is_asset_archive("dlc/readme.zip"));
    assert!(!p.is_asset_archive("common/big.zip"));
}

#[test]
fn shipped_profile_exposes_locations_and_stores() {
    let p = shipped();
    assert_eq!(p.steam_id(), Some(281_990));
    let l = p.locations();
    assert!(l.user_dir["windows"].starts_with("{DOCUMENTS}/"));
    assert!(l.workshop.as_deref().unwrap().contains("{STEAM_LIB}"));
    assert_eq!(l.steam_library_index.len(), 2);
    let s = p.stores();
    assert_eq!(s.executable.as_deref(), Some("stellaris.exe"));
    assert!(s.xbox_install.as_deref().unwrap().starts_with("XboxGames/"));
    assert!(!s.extra_user_dirs.is_empty());
}

#[test]
fn unknown_keys_in_touched_tables_are_load_errors() {
    for bad in [
        "[locations]\nworkshp = \"x\"\n",
        "[stores]\nexecutible = \"x\"\n",
        "[game]\nstem_id = 1\n",
        "[locations.user_dir]\nbeos = \"x\"\n",
    ] {
        assert!(Profile::from_toml(bad).is_err(), "{bad}");
    }
    assert!(Profile::from_toml("[locations.user_dir]\nwindows = \"x\"\n").is_ok());
}

#[test]
fn dir_classes_come_from_the_shipped_profile() {
    use smd_core::{FileKind, FileOrder, MergeMode};
    let p = shipped();
    let t = p.dir_class("common/traits/00_traits.txt");
    assert_eq!((t.kind, t.extractor, t.level), (FileKind::Script, "depth_key", 0));
    assert_eq!(
        (t.merge, t.order, t.domain_name.as_deref()),
        (MergeMode::MergeByKey, Some(FileOrder::Fios), Some("trait"))
    );
    assert_eq!(p.dir_class("common/defines/00_defines.txt").level, 1);
    assert_eq!(p.dir_class("events/a.txt").extractor, "pdx_event");
    assert_eq!(p.dir_class("gfx/a.dds").kind, FileKind::Asset);
    assert_eq!(p.dir_class("nowhere/a.txt").kind, FileKind::Unknown);
    let d = t.domain.unwrap();
    assert_eq!(&*p.domain_names()[usize::from(d.0)], "trait");
}
