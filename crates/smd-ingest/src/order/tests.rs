use std::fs;
use std::sync::Arc;
use std::time::Duration;

use smd_core::{Confidence, LoadOrderSource, OrderError, SnapshotMethod};

use super::launcher_v2::fixture::{Launcher, PosKind};
use super::*;
use crate::testutil::World;

fn opts(w: &World) -> SourceOptions {
    let launcher =
        LauncherV2Options { backoff: Duration::from_millis(5), ..LauncherV2Options::default() };
    SourceOptions { discovery: w.options(), launcher }
}

fn five(w: &World) -> [&'static str; 5] {
    let ids = ["111", "222", "333", "444", "555"];
    for id in ids {
        w.workshop_mod(id, id);
    }
    ids
}

fn dlc_json(w: &World, listed: &[&str]) {
    let items: Vec<String> = listed.iter().map(|i| format!("\"mod/ugc_{i}.mod\"")).collect();
    let body = format!("{{\"disabled_dlcs\": [], \"enabled_mods\": [{}]}}", items.join(", "));
    w.write_user("dlc_load.json", body.as_bytes());
}

fn activation(p: &Playset, key: &str) -> Option<(u32, bool)> {
    p.entries.iter().find(|e| e.key.as_str() == key).map(|e| (e.position, e.enabled))
}

use smd_core::Playset;

#[test]
fn dlc_load_json_order_and_activation() {
    let w = World::new();
    five(&w);
    dlc_json(&w, &["333", "111", "555"]);
    let src = DlcLoadJson::new(w.options());
    assert_eq!(src.ceiling(), (Confidence::Inferred, Confidence::Inferred));
    let p = src.try_load(&w.locations()).expect("reads");
    assert_eq!(
        (p.order, p.activation, p.snapshot),
        (Confidence::Inferred, Confidence::Inferred, SnapshotMethod::NotApplicable)
    );
    assert_eq!(activation(&p, "333"), Some((0, true)));
    assert_eq!(activation(&p, "111"), Some((1, true)));
    assert_eq!(activation(&p, "555"), Some((2, true)));
    assert_eq!(p.entries.len(), 3, "unlisted mods are absent, hence Inactive downstream");
    let chain = OrderChain::new(vec![Box::new(src)]);
    let inv = crate::inventory(&w.locations(), &chain, &w.options());
    let state = |k: &str| {
        let (id, _) = inv.table.iter().find(|(_, m)| m.key.as_str() == k).expect(k);
        let m = inv.table.get(id).unwrap();
        (m.activation, m.rank.clone())
    };
    assert_eq!(state("333"), (smd_core::Activation::Active, vec![2, 0]));
    assert_eq!(state("555"), (smd_core::Activation::Active, vec![2, 2]));
    assert_eq!(state("222"), (smd_core::Activation::Inactive, vec![]));
    assert_eq!(state("444"), (smd_core::Activation::Inactive, vec![]));
    let prov = inv.provenance;
    assert_eq!(
        (prov.order_source, prov.order, prov.activation, prov.snapshot),
        (
            "dlc_load_json",
            Confidence::Inferred,
            Confidence::Inferred,
            SnapshotMethod::NotApplicable
        )
    );
}

#[test]
fn an_unmatched_dlc_entry_is_reported_and_duplicates_collapse() {
    let w = World::new();
    five(&w);
    dlc_json(&w, &["111", "999", "111"]);
    let attempt = DlcLoadJson::new(w.options()).load(&w.locations());
    assert_eq!(attempt.result.expect("reads").entries.len(), 1);
    assert!(attempt.diagnostics.iter().any(|d| d.code == codes::PLAYSET_MOD_UNMATCHED));
}

fn nested(depth: usize) -> String {
    format!("{{\"enabled_mods\": {}{}}}", "[".repeat(depth), "]".repeat(depth))
}

#[test]
fn h24_dlc_load_json_caps() {
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("depth 65", nested(64).into_bytes()),
        ("20001 entries", {
            let items = vec!["\"mod/x.mod\""; 20_001].join(",");
            format!("{{\"enabled_mods\": [{items}]}}").into_bytes()
        }),
        ("over 16 MiB", {
            let mut b = b"{\"enabled_mods\": [], \"pad\": \"".to_vec();
            b.resize(16 * 1024 * 1024 + 1, b'a');
            b.extend_from_slice(b"\"}");
            b
        }),
    ];
    for (what, bytes) in cases {
        let w = World::new();
        five(&w);
        w.write_user("dlc_load.json", &bytes);
        let attempt = DlcLoadJson::new(w.options()).load(&w.locations());
        assert!(matches!(attempt.result, Err(OrderError::Unreadable { .. })), "{what}");
        assert!(attempt.diagnostics.iter().any(|d| d.code == codes::DLC_LOAD_INVALID), "{what}");
        // The chain falls through to DirectoryScan.
        let chain = OrderChain::standard(&opts(&w));
        let out = chain.run(&w.locations());
        assert_eq!(out.order_source, "directory_scan", "{what}");
        assert!(out.diagnostics.iter().any(|d| d.code == codes::DLC_LOAD_INVALID), "{what}");
        assert!(out.diagnostics.iter().any(|d| d.code == codes::ORDER_SOURCE_FAILED), "{what}");
    }
}

#[test]
fn a_non_string_enabled_entry_is_diagnosed_not_skipped_silently() {
    let w = World::new();
    five(&w);
    w.write_user("dlc_load.json", br#"{"enabled_mods": ["mod/ugc_111.mod", 7, null]}"#);
    let attempt = DlcLoadJson::new(w.options()).load(&w.locations());
    assert_eq!(attempt.result.expect("reads").entries.len(), 1);
    let bad: Vec<_> =
        attempt.diagnostics.iter().filter(|d| d.code == codes::DLC_LOAD_INVALID).collect();
    assert_eq!(bad.len(), 2, "{bad:?}");
}

#[test]
fn depth_64_and_20000_entries_are_accepted() {
    assert!(json_caps::check_depth(nested(63).as_bytes()).is_ok());
    assert!(json_caps::check_depth(nested(64).as_bytes()).is_err());
    assert!(
        json_caps::check_depth(b"{\"a\": \"[[[[[[[[[[\\\"[[[\"}").is_ok(),
        "brackets in strings do not count"
    );
    let items = vec!["1"; 20_000].join(",");
    let v: serde_json::Value = serde_json::from_str(&format!("[{items}]")).unwrap();
    assert!(json_caps::check_entries(&v).is_ok());
}

#[test]
fn wrong_shapes_are_schema_unknown_or_corrupt() {
    let w = World::new();
    five(&w);
    w.write_user("dlc_load.json", br#"{"something_else": 1}"#);
    let src = DlcLoadJson::new(w.options());
    assert!(matches!(src.try_load(&w.locations()), Err(OrderError::SchemaUnknown { .. })));
    w.write_user("dlc_load.json", b"{not json");
    assert!(matches!(src.try_load(&w.locations()), Err(OrderError::Corrupt { .. })));
    fs::remove_file(w.user.join("dlc_load.json")).unwrap();
    assert_eq!(src.try_load(&w.locations()), Err(OrderError::NotPresent));
}

#[test]
fn directory_scan_is_fallback_for_every_mod() {
    let w = World::new();
    five(&w);
    let src = DirectoryScan::new(w.options());
    assert_eq!(src.ceiling(), (Confidence::Fallback, Confidence::Fallback));
    let p = src.try_load(&w.locations()).expect("always answers");
    assert_eq!(p.entries.len(), 5);
    assert_eq!((p.order, p.activation), (Confidence::Fallback, Confidence::Fallback));
}

#[test]
fn unknown_source_names_are_a_diagnostic() {
    let specs =
        [smd_core::SourceSpec::named("directory_scan"), smd_core::SourceSpec::named("telepathy")];
    let (chain, diags) = OrderChain::from_specs(&specs, &SourceOptions::default());
    assert_eq!(chain.sources.len(), 1);
    assert_eq!(diags[0].code, codes::UNKNOWN_ORDER_SOURCE);
}

#[test]
fn an_empty_chain_says_so() {
    let w = World::new();
    let out = OrderChain::new(Vec::new()).run(&w.locations());
    assert_eq!(
        (out.order_source, out.playset.order, out.playset.activation),
        ("none", Confidence::Fallback, Confidence::Fallback)
    );
    assert!(out.diagnostics.iter().any(|d| d.code == codes::NO_ORDER_SOURCE));
}

#[test]
fn activation_disagreement_reported() {
    let w = World::new();
    let ids = ["111", "222", "333", "444"];
    for id in ids {
        w.workshop_mod(id, id);
    }
    let l = Launcher::create(&w.user, PosKind::Int);
    for id in ids {
        l.mod_row(id);
    }
    l.playset("p1", true, false);
    l.fill("p1", &[("111", true), ("222", true), ("333", true), ("444", false)]);
    dlc_json(&w, &["111", "222", "444"]);
    let chain = OrderChain::standard(&opts(&w));
    let inv = crate::inventory(&w.locations(), &chain, &w.options());
    assert_eq!(inv.provenance.order_source, "launcher_v2_sqlite");
    let d: Vec<_> =
        inv.diagnostics.iter().filter(|d| d.code == codes::ACTIVATION_DISAGREEMENT).collect();
    assert_eq!(d.len(), 1, "{:?}", inv.diagnostics);
    let text = &d[0].detail;
    assert!(text.contains("only per launcher_v2_sqlite: [333]"), "{text}");
    assert!(text.contains("only per dlc_load_json: [444]"), "{text}");
    for agreed in ["111", "222"] {
        assert!(!text.contains(agreed), "{agreed} is not part of the disagreement: {text}");
    }
    let active = |k: &str| {
        let (id, _) = inv.table.iter().find(|(_, m)| m.key.as_str() == k).unwrap();
        inv.table.get(id).unwrap().activation
    };
    assert_eq!(active("333"), smd_core::Activation::Active, "the DB wins");
    assert_eq!(active("444"), smd_core::Activation::Inactive, "the DB wins");
    drop(l);
}

#[test]
fn agreeing_sources_raise_no_disagreement() {
    let w = World::new();
    let ids = ["111", "222"];
    for id in ids {
        w.workshop_mod(id, id);
    }
    let l = Launcher::create(&w.user, PosKind::Int);
    l.seed(&ids);
    dlc_json(&w, &ids);
    let out = OrderChain::standard(&opts(&w)).run(&w.locations());
    assert!(out.diagnostics.iter().all(|d| d.code != codes::ACTIVATION_DISAGREEMENT));
    assert_eq!(out.order_source, "launcher_v2_sqlite");
}

#[test]
fn h20_torn_copy_falls_through_to_dlc_load_json() {
    let w = World::new();
    let ids = ["111", "222"];
    for id in ids {
        w.workshop_mod(id, id);
    }
    let l = Launcher::create(&w.user, PosKind::Int);
    l.seed(&ids);
    l.checkpoint();
    l.reorder(&["222", "111"]);
    dlc_json(&w, &["111", "222"]);
    let wal = l.path.with_file_name("launcher-v2.sqlite-wal");
    let hook: Arc<dyn Fn(u32) + Send + Sync> = Arc::new(move |_| {
        use std::io::Write;
        fs::OpenOptions::new().append(true).open(&wal).unwrap().write_all(b"x").unwrap();
    });
    let mut o = opts(&w);
    o.launcher.allow_online = false;
    o.launcher.hooks.between_passes = Some(hook);
    let out = OrderChain::standard(&o).run(&w.locations());
    assert_eq!(out.order_source, "dlc_load_json");
    let d = out
        .diagnostics
        .iter()
        .find(|d| d.code == codes::LAUNCHER_SNAPSHOT_UNVERIFIED)
        .expect("unverified");
    assert!(d.detail.contains("double_pass"), "{}", d.detail);
    assert_eq!(
        (out.playset.order, out.playset.activation),
        (Confidence::Inferred, Confidence::Inferred)
    );
}
