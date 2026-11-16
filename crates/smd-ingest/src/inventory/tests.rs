use std::fs;

use smd_core::{
    Activation, Confidence, ContentIncompleteReason, SnapshotMethod, SourceId, SourceRole,
};

use crate::codes;
use crate::order::{OrderChain, SourceOptions};
use crate::testutil::World;

use super::*;

fn scan_only(w: &World) -> Inventory {
    let chain =
        OrderChain::standard(&SourceOptions { discovery: w.options(), ..SourceOptions::default() });
    inventory(&w.locations(), &chain, &w.options())
}

#[test]
fn h16_source_ids_canonical() {
    let mut tables = Vec::new();
    for creation in [["a", "b", "c"], ["c", "a", "b"]] {
        let w = World::new();
        for stem in creation {
            let dir = w.local_mod(stem, &format!("Mod {stem}"));
            fs::write(dir.join("marker.txt"), stem).unwrap();
        }
        tables.push(scan_only(&w).table);
    }
    assert_eq!(tables[0], tables[1]);
    let t = &tables[0];
    assert_eq!(t.len(), 4);
    let vanilla = t.get(SourceId::VANILLA).unwrap();
    assert_eq!((vanilla.role, vanilla.rank.clone()), (SourceRole::Vanilla, vec![0]));
    let keys: Vec<&str> = t.iter().skip(1).map(|(_, m)| m.key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    assert_eq!(keys, sorted, "mods follow vanilla in ModKey byte order");
}

#[test]
fn h21_directory_scan_never_active() {
    let w = World::new();
    for id in ["111", "222", "333"] {
        w.workshop_mod(id, id);
    }
    let inv = scan_only(&w);
    let p = inv.provenance;
    assert_eq!(
        (p.order_source, p.order, p.activation),
        ("directory_scan", Confidence::Fallback, Confidence::Fallback)
    );
    assert_eq!(p.snapshot, SnapshotMethod::NotApplicable);
    for (id, m) in inv.table.iter().filter(|(id, _)| *id != SourceId::VANILLA) {
        assert_eq!(m.activation, Activation::Unknown, "{}", m.key);
        assert!(m.rank.is_empty(), "{}", m.key);
        assert_ne!(m.activation, Activation::Active, "{id:?}");
    }
    let vanilla = inv.table.get(SourceId::VANILLA).unwrap();
    assert_eq!(
        vanilla.activation,
        Activation::Active,
        "vanilla is the one thing that always loads"
    );
}

#[test]
fn archive_mod_inventoried_unread() {
    let w = World::new();
    w.mod_file("zipped", "name=\"Zipped\"\narchive=\"mod/zipped.zip\"\nremote_file_id=\"42\"\n");
    let inv = scan_only(&w);
    let (id, meta) = inv.table.iter().find(|(_, m)| m.key.as_str() == "42").expect("inventoried");
    assert_eq!(meta.role, SourceRole::Archive);
    let rec = &inv.records[id.index() as usize];
    assert_eq!(rec.content, ContentState::Unread(UnreadReason::Archive));
    assert_eq!(rec.content_root, None);
    assert_eq!(
        inv.completeness.content_incomplete,
        vec![(rec.key.clone(), ContentIncompleteReason::ArchiveUnread, None)]
    );
}

#[test]
fn h24_a_rejected_descriptor_stays_in_the_inventory_under_its_directory_name() {
    let mut cases: Vec<(&str, Vec<u8>)> = Vec::new();
    let mut big = b"name=\"x\"\n".to_vec();
    big.resize(65 * 1024, b'\n');
    cases.push(("65 KiB", big));
    cases.push(("5 KiB value", format!("name=\"{}\"", "v".repeat(5 * 1024)).into_bytes()));
    cases.push(("257 tags", format!("tags={{ {} }}", "\"t\" ".repeat(257)).into_bytes()));
    cases.push(("depth 9", format!("{}{}", "k={".repeat(9), "}".repeat(9)).into_bytes()));
    for (what, bytes) in cases {
        let w = World::new();
        fs::write(w.user.join("mod").join("victim.mod"), bytes).unwrap();
        let inv = scan_only(&w);
        assert_eq!(inv.table.len(), 2, "{what}");
        let meta = inv.table.get(SourceId(1)).unwrap();
        let rec = &inv.records[1];
        assert_eq!(&*meta.name, "victim", "{what}");
        assert!(matches!(rec.content, ContentState::Unread(_)), "{what}");
        assert_eq!(rec.content_root, None, "{what}");
        assert_eq!(rec.content, ContentState::Unread(UnreadReason::DescriptorInvalid), "{what}");
        let invalid = rec.diagnostics.iter().any(|d| d.code == codes::DESCRIPTOR_INVALID);
        assert!(invalid, "{what}: {:?} {:?}", rec.diagnostics, inv.diagnostics);
        assert_eq!(inv.completeness.content_incomplete.len(), 1, "{what}");
    }
}

#[test]
fn descriptor_diagnostics_are_sited_at_the_assigned_source() {
    let w = World::new();
    w.mod_file("bad", &format!("name=\"{}\"", "v".repeat(5000)));
    let inv = scan_only(&w);
    let d = &inv.records[1].diagnostics[0];
    assert_eq!(d.code, codes::DESCRIPTOR_INVALID);
    assert_eq!(d.site.source, SourceId(1));
}

#[test]
fn vanilla_is_content_incomplete_when_there_is_no_install() {
    let w = World::new();
    let mut loc = w.locations();
    loc.insert(crate::KEY_INSTALL, w.tmp.path().join("gone"));
    let chain =
        OrderChain::standard(&SourceOptions { discovery: w.options(), ..SourceOptions::default() });
    let inv = inventory(&loc, &chain, &w.options());
    let vanilla = &inv.records[0];
    assert_eq!(vanilla.content, ContentState::Unread(UnreadReason::NoPath));
    let entry =
        inv.completeness.content_incomplete.iter().find(|(k, _, _)| k.as_str() == "vanilla");
    assert!(entry.is_some(), "{:?}", inv.completeness);
    assert!(inv.diagnostics.iter().any(|d| d.code == codes::LOCATION_MISSING));
    let ok = scan_only(&w);
    assert!(ok.completeness.content_incomplete.is_empty());
    assert!(!ok.discovery_incomplete);
}
