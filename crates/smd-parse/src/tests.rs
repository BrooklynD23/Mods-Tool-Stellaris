//! One smoke check per extractor shape. Broad suites and the golden corpus are deferred
//! (docs/HANDOFF.md § M2).

use smd_core::{DomainId, FactPayload, FileKind, MergeMode, ParseCompleteness};
use smd_profiles::DirClass;

use crate::{ParseInput, ParseOutput, parse_file};

fn class(extractor: &'static str, level: u8, children: &[&str]) -> DirClass {
    DirClass {
        kind: FileKind::Script,
        parser: "clausewitz",
        extractor,
        level,
        domain: Some(DomainId(3)),
        domain_name: Some("thing".into()),
        merge: MergeMode::MergeByKey,
        order: None,
        tier: 0,
        additive_children: children.iter().map(|c| (*c).into()).collect(),
    }
}

fn run(text: &str, class: &DirClass) -> ParseOutput {
    parse_file(&ParseInput { bytes: text.as_bytes(), class, cancelled: &|| false })
}

fn symbols(out: &ParseOutput) -> Vec<String> {
    out.facts
        .iter()
        .filter_map(|f| {
            if let FactPayload::Definition { symbol, .. } = &f.payload {
                Some(symbol.to_string())
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn depth_key_levels_bodies_and_recovery() {
    let src = "\u{feff}@x = 1\na = { b = 1 } # c\n\"q\" = yes\nc > 3 d = rgb { 1 2 3 }";
    let out = run(src, &class("depth_key", 0, &[]));
    assert_eq!(symbols(&out), ["a", "q", "d"]);
    assert_eq!(out.completeness, ParseCompleteness::Complete);
    let reformatted = run("a = {\n  b   =   1\n}", &class("depth_key", 0, &[]));
    assert_eq!(out.facts[0].payload, reformatted.facts[0].payload, "whitespace is not a change");
    let defines = run("NG = { K = 1 J = { x } }", &class("depth_key", 1, &[]));
    assert_eq!(symbols(&defines), ["NG.K", "NG.J"]);
    let keys = run("NG = { K = 1 }", &class("define", 1, &[]));
    assert!(matches!(&keys.facts[..], [f] if matches!(&f.payload,
        FactPayload::DefineKey { group, key, .. } if &**group == "NG" && &**key == "K")));
    let open = run("a = { b = { c = 1 }", &class("depth_key", 0, &[]));
    assert_eq!((symbols(&open), open.completeness), (vec!["a".into()], ParseCompleteness::Partial));
}

#[test]
fn deep_nesting_is_capped_not_recursed() {
    let deep = format!("{}{}", "a = { ".repeat(10_000), "} ".repeat(10_000));
    let out = run(&format!("{deep} z = 1"), &class("depth_key", 0, &[]));
    assert_eq!(symbols(&out), ["a", "z"]);
    assert_eq!(out.completeness, ParseCompleteness::Partial);
    assert!(out.diags.iter().any(|d| &*d.code == crate::script::codes::DEPTH_LIMIT));
}

#[test]
fn additive_split_and_events() {
    let c = class("depth_key", 0, &["events"]);
    let a = run("on_x = { events = { e.1 } effect = { y } }", &c);
    let b = run("on_x = { events = { e.2 e.3 } effect = { y } }", &c);
    let residual = |o: &ParseOutput| match &o.facts[1].payload {
        FactPayload::AdditiveSplit { residual_hash, additive_count, .. } => {
            (*residual_hash, *additive_count)
        }
        other => panic!("{other:?}"),
    };
    assert_eq!(residual(&a), residual(&b));
    assert_eq!(residual(&a).1, 1);
    let ev = run(
        "namespace = my\nmy.1 = { id = my.1 } x = { id = my.setup }",
        &class("pdx_event", 0, &[]),
    );
    assert_eq!(ev.facts.len(), 3);
}
