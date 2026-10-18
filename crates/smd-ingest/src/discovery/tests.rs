use std::fs;

use smd_core::{DescriptorSource, SourceRole};

use super::*;
use crate::codes;
use crate::testutil::World;

#[test]
fn local_and_workshop_mods_are_found_with_keys() {
    let w = World::new();
    w.local_mod("alpha", "Alpha");
    w.workshop_mod("2001", "Beta");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.install.as_deref(), Some(w.install.as_path()));
    let keys: Vec<&str> = d.mods.iter().map(|m| m.key.as_str()).collect();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"2001"), "workshop id is the key: {keys:?}");
    assert!(keys.iter().any(|k| k.starts_with("h-")), "local mods are hashed: {keys:?}");
    let ws = d.mods.iter().find(|m| m.key.as_str() == "2001").expect("workshop mod");
    assert_eq!(ws.role, SourceRole::Workshop);
    assert_eq!(ws.content, ContentState::Readable);
    assert_eq!(ws.mod_file.as_deref(), Some("ugc_2001.mod"));
}

#[test]
fn unclaimed_workshop_directories_are_read_from_their_descriptor() {
    let w = World::new();
    let dir = w.workshop.join("777");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("descriptor.mod"), "name=\"Direct\"\nremote_file_id=\"777\"\n").unwrap();
    // A claimed one must not appear twice.
    w.workshop_mod("888", "Claimed");
    fs::write(w.workshop.join("888").join("descriptor.mod"), "name=\"Claimed\"\n").unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 2, "{:?}", d.mods.iter().map(|m| &m.key).collect::<Vec<_>>());
    let direct = d.mods.iter().find(|m| m.key.as_str() == "777").expect("direct");
    assert_eq!(&*direct.name, "Direct");
    assert_eq!(direct.mod_file, None);
}

#[test]
fn a_rejected_descriptor_is_inventoried_under_its_directory_name() {
    let w = World::new();
    w.mod_file("broken_one", &format!("name=\"{}\"\n", "x".repeat(5000)));
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1);
    let m = &d.mods[0];
    assert_eq!(&*m.name, "broken_one");
    assert_eq!(m.content, ContentState::Unread(UnreadReason::DescriptorInvalid));
    assert_eq!(m.content_root, None);
    assert!(m.diagnostics.iter().any(|x| x.code == codes::DESCRIPTOR_INVALID));
}

#[test]
fn an_oversize_launcher_file_is_inventoried_not_dropped() {
    let w = World::new();
    let mut body = b"name=\"big\"\n".to_vec();
    body.resize(65 * 1024, b' ');
    fs::write(w.user.join("mod").join("big.mod"), body).unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1);
    assert_eq!(&*d.mods[0].name, "big");
    assert!(matches!(d.mods[0].content, ContentState::Unread(_)));
    assert_eq!(d.mods[0].content, ContentState::Unread(UnreadReason::DescriptorInvalid));
    assert!(d.mods[0].diagnostics.iter().any(|x| x.code == codes::DESCRIPTOR_INVALID));
    assert!(!d.diagnostics.iter().any(|x| x.code == codes::DESCRIPTOR_UNREADABLE));
}

#[test]
fn descriptor_path_root_refused() {
    let w = World::new();
    w.mod_file("slash", "name=\"Slash\"\npath=\"/\"\n");
    w.mod_file("homey", &format!("name=\"Home\"\npath=\"{}\"\n", w.home.display()));
    w.mod_file(
        "above",
        &format!("name=\"Above\"\npath=\"{}\"\n", w.user.parent().unwrap().display()),
    );
    w.mod_file("game", &format!("name=\"Game\"\npath=\"{}\"\n", w.install.display()));
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 4);
    for m in &d.mods {
        assert_eq!(m.content, ContentState::Unread(UnreadReason::RootRefused), "{}", m.name);
        assert_eq!(m.content_root, None, "{} must not be walked", m.name);
    }
    let refused = d.diagnostics.iter().filter(|x| x.code == codes::ROOT_REFUSED).count();
    assert_eq!(refused, 4);
}

#[test]
fn a_missing_path_is_a_diagnostic_not_a_walk() {
    let w = World::new();
    w.mod_file("gone", "name=\"Gone\"\npath=\"mod/nowhere\"\n");
    w.mod_file("none", "name=\"None\"\n");
    let d = discover(&w.locations(), &w.options());
    assert!(d.mods.iter().all(|m| m.content_root.is_none()));
    assert_eq!(d.diagnostics.iter().filter(|x| x.code == codes::MOD_PATH_MISSING).count(), 2);
}

#[test]
fn duplicate_workshop_ids_get_distinct_keys() {
    let w = World::new();
    w.mod_file("one", "name=\"One\"\nremote_file_id=\"5\"\npath=\"mod/one\"\n");
    w.mod_file("two", "name=\"Two\"\nremote_file_id=\"5\"\npath=\"mod/two\"\n");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 2);
    assert_ne!(d.mods[0].key, d.mods[1].key);
    assert!(d.diagnostics.iter().any(|x| x.code == codes::DUPLICATE_KEY));
}

#[test]
fn guard_resolves_relative_paths_against_the_user_dir() {
    let w = World::new();
    let dir = w.user.join("mod").join("rel");
    fs::create_dir_all(&dir).unwrap();
    w.mod_file("rel", "name=\"Rel\"\npath=\"mod/rel\"\n");
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods[0].content_root.as_deref(), Some(dir.as_path()));
}

#[cfg(unix)]
#[test]
fn a_linked_mod_file_is_a_diagnostic_and_an_incomplete_scan() {
    let w = World::new();
    w.local_mod("real", "Real");
    let target = w.tmp.path().join("elsewhere.mod");
    fs::write(&target, "name=\"Linked\"\n").unwrap();
    std::os::unix::fs::symlink(&target, w.user.join("mod").join("linked.mod")).unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1, "the link is not followed");
    assert!(d.incomplete);
    let gap = d.diagnostics.iter().find(|x| x.code == codes::DISCOVERY_INCOMPLETE);
    assert!(gap.expect("reported").subject.contains("linked.mod"));
}

#[cfg(unix)]
#[test]
fn a_linked_descriptor_in_a_workshop_dir_is_reported() {
    let w = World::new();
    let dir = w.workshop.join("42");
    fs::create_dir_all(&dir).unwrap();
    let target = w.tmp.path().join("d.mod");
    fs::write(&target, "name=\"X\"\n").unwrap();
    std::os::unix::fs::symlink(&target, dir.join("descriptor.mod")).unwrap();
    let d = discover(&w.locations(), &w.options());
    assert!(d.incomplete);
    assert!(d.diagnostics.iter().any(|x| x.code == codes::DISCOVERY_INCOMPLETE));
    assert_eq!(d.mods.len(), 1, "still inventoried by directory name");
    assert_eq!(d.mods[0].content, ContentState::Unread(UnreadReason::DescriptorUnreadable));
}

#[test]
fn a_workshop_dir_without_a_descriptor_is_inventoried_with_a_diagnostic() {
    let w = World::new();
    fs::create_dir_all(w.workshop.join("9001")).unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1);
    let m = &d.mods[0];
    assert_eq!(&*m.dir_name, "9001");
    assert_eq!(m.role, SourceRole::Workshop);
    assert_eq!(m.content, ContentState::Unread(UnreadReason::DescriptorUnreadable));
    assert_eq!(m.content_root, None);
    assert!(d.diagnostics.iter().any(|x| x.code == codes::DESCRIPTOR_UNREADABLE));
}

#[test]
fn a_workshop_descriptor_is_sourced_as_workshop() {
    let w = World::new();
    let dir = w.workshop.join("777");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("descriptor.mod"), "name=\"Direct\"\n").unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods[0].descriptor.source, DescriptorSource::Workshop);
}

#[test]
fn a_given_location_that_is_not_there_is_named_and_makes_the_scan_incomplete() {
    let w = World::new();
    let mut loc = w.locations();
    loc.insert(KEY_INSTALL, w.tmp.path().join("no-install"));
    loc.insert(KEY_WORKSHOP, w.tmp.path().join("no-workshop"));
    let d = discover(&loc, &w.options());
    let missing: Vec<&str> = d
        .diagnostics
        .iter()
        .filter(|x| x.code == codes::LOCATION_MISSING)
        .map(|x| &*x.subject)
        .collect();
    assert_eq!(missing, [KEY_INSTALL, KEY_WORKSHOP]);
    assert_eq!(d.install, None);
    assert!(d.incomplete);
}

#[test]
fn a_complete_scan_is_not_flagged_incomplete() {
    let w = World::new();
    w.local_mod("a", "A");
    assert!(!discover(&w.locations(), &w.options()).incomplete);
}

#[test]
fn a_rejected_launcher_file_still_claims_its_workshop_directory() {
    let w = World::new();
    let dir = w.workshop.join("555");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("descriptor.mod"), "name=\"Same\"\nremote_file_id=\"555\"\n").unwrap();
    let mut big = b"name=\"Same\"\n".to_vec();
    big.resize(65 * 1024, b' ');
    fs::write(w.user.join("mod").join("ugc_555.mod"), big).unwrap();
    let d = discover(&w.locations(), &w.options());
    assert_eq!(d.mods.len(), 1, "{:?}", d.mods.iter().map(|m| &m.dir_name).collect::<Vec<_>>());
    assert_eq!(d.mods[0].content, ContentState::Unread(UnreadReason::DescriptorInvalid));
}
