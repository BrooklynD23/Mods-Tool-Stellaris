//! Gate H25: every mod-supplied string is inert in its sink.

mod common;

use common::*;
use smd_report::{Json, Markdown, RenderOptions, Renderer, Table};

const OSC: &str = "\x1b]8;;http://e.x\x07Evil\x1b]8;;\x07";
const BIDI_PATH: &str = "common/traits/gnp\u{202e}txt.exe";

fn hostile(name: &str, path: &str) -> smd_report::ReportModel {
    let f = finding(path, vec![participant("k_1", name, path, smd_core::Role::Winner)]);
    model(complete(), vec![row("k_1", name, path)], vec![f])
}

#[test]
fn h25_terminal_inert() {
    let out = Table.render(&hostile(OSC, BIDI_PATH), &RenderOptions::default());
    assert!(!out.bytes().any(|b| b == 0x1b || b == 0x07), "raw control byte in {out:?}");
    assert!(!out.contains('\u{202e}'));
    assert!(out.contains("\\u{1b}") && out.contains("\\u{202e}"));
    assert!(out.contains("\\u{7}"));
}

#[test]
fn h25_terminal_colour_is_the_tools_and_only_on_request() {
    let m = hostile(OSC, BIDI_PATH);
    let plain = Table.render(&m, &RenderOptions::default());
    assert!(!plain.contains('\x1b'));
    let tty = Table.render(&m, &RenderOptions::default().with_color(true));
    assert!(tty.contains("\x1b[1m"));
    // Data still carries no raw ESC beyond the tool's own `ESC [` styles.
    assert!(
        !tty.replace("\x1b[1m", "").replace("\x1b[33m", "").replace("\x1b[0m", "").contains('\x1b')
    );
}

#[test]
fn h25_markdown_inert() {
    let opts = RenderOptions::default();
    let link = Markdown.render(&hostile("[Fix](https://e.x)", "a.txt"), &opts);
    assert!(!link.contains("]("), "{link}");
    assert!(link.contains("\\[Fix\\]"));
    assert!(!link.contains("https://e.x)"));

    let html = Markdown.render(&hostile("<script>alert(1)</script>", "a.txt"), &opts);
    assert!(!html.contains('<') && !html.contains('>'), "{html}");
    assert!(html.contains("&lt;script&gt;"));

    let ticks = "x`````y";
    let m = hostile("Ticks", &format!("common/{ticks}.txt"));
    let out = Markdown.render(&m, &opts);
    assert!(out.contains(&format!("{}common/{ticks}.txt{}", "`".repeat(6), "`".repeat(6))));

    let named = Markdown.render(&hostile("a`````b", "a.txt"), &opts);
    assert!(named.contains(&format!("{}a`````b{}", "`".repeat(6), "`".repeat(6))), "{named}");

    let ctl = Markdown.render(&hostile(OSC, BIDI_PATH), &opts);
    assert!(!ctl.contains('\x1b') && !ctl.contains('\x07') && !ctl.contains('\u{202e}'));
    assert!(ctl.contains("u{202e}"));
}

#[test]
fn h25_markdown_pipes_do_not_break_table_rows() {
    let out = Markdown.render(&hostile("a|b", "x|y.txt"), &RenderOptions::default());
    let inv = out.lines().find(|l| l.starts_with("| `k_1`")).expect("inventory row");
    // 8 cells => 9 unescaped pipes.
    assert_eq!(inv.replace("\\|", "").matches('|').count(), 9, "{inv}");
}

#[test]
fn h25_json_valid_roundtrip() {
    let hostile_strings = [
        OSC,
        BIDI_PATH,
        "[Fix](https://e.x)",
        "<script>alert(1)</script>",
        "x`````y",
        "line\nbreak\ttab \"quote\" back\\slash",
        "nul\u{0}del\u{7f}c1\u{85}\u{2028}",
    ];
    let mut f = pair("a.txt");
    for (i, s) in hostile_strings.iter().enumerate() {
        f.participants.push(participant(&format!("k_{i}"), s, s, smd_core::Role::Contributor));
    }
    let rows = hostile_strings.iter().map(|s| row(s, s, s)).collect();
    let m = model(complete(), rows, vec![f]);
    let opts = RenderOptions::default()
        .with_caps(smd_report::Caps { max_participants: 99, ..Default::default() });
    let out = Json.render(&m, &opts);
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    let inv = v["header"]["inventory"].as_array().expect("inventory");
    for (s, r) in hostile_strings.iter().zip(inv) {
        assert_eq!(r["name"].as_str(), Some(*s));
        assert_eq!(r["path"].as_str(), Some(*s));
    }
    let parts = v["findings"][0]["participants"].as_array().expect("participants");
    for (s, p) in hostile_strings.iter().zip(&parts[2..]) {
        assert_eq!(p["mod_name"].as_str(), Some(*s));
        assert_eq!(p["path"].as_str(), Some(*s));
    }
}

#[test]
fn h25_invisible_characters_are_visible_in_table_and_markdown() {
    let sneaky = "ev\u{200b}il\u{feff}\u{e0041}";
    let m = hostile(sneaky, "a.txt");
    for (name, out) in [
        ("table", Table.render(&m, &RenderOptions::default())),
        ("markdown", Markdown.render(&m, &RenderOptions::default())),
    ] {
        assert!(!out.contains('\u{200b}') && !out.contains('\u{feff}'), "{name}");
        assert!(!out.contains('\u{e0041}') && out.contains("200b"), "{name}");
    }
}

#[test]
fn h25_json_escapes_what_a_terminal_would_act_on() {
    let out =
        Json.render(&hostile("a\u{9b}b\u{202e}c\u{200b}", "a.txt"), &RenderOptions::default());
    assert!(!out.contains('\u{9b}') && !out.contains('\u{202e}') && !out.contains('\u{200b}'));
    let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
    assert_eq!(v["header"]["inventory"][0]["name"].as_str(), Some("a\u{9b}b\u{202e}c\u{200b}"));
}
