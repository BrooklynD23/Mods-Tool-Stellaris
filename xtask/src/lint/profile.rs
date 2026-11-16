//! The `profile-audit` gate.
//!
//! `panic = "unwind"` is load-bearing: rule isolation is built on `catch_unwind`, and
//! under `panic = "abort"` that call is a no-op, so one panicking rule takes the whole
//! scan down. Size pressure pushes toward `abort` — green CI, dead user — which is why the
//! setting is audited rather than trusted (ADR-0001, REVIEW-2026-08 § G18).
//!
//! The same gate checks that the two size limits this linter *delegates* are still wired
//! up: `too_many_lines` and `too_many_arguments` denied in `[workspace.lints.clippy]`,
//! with their thresholds in `clippy.toml`. A delegated gate that quietly stops being
//! configured is worse than no gate, because the workspace still claims to have it.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Ctx, Violation};

const RULE: &str = "profile-audit";

#[derive(Deserialize)]
struct Root {
    #[serde(default)]
    profile: BTreeMap<String, BTreeMap<String, toml::Value>>,
    #[serde(default)]
    workspace: Workspace,
}

#[derive(Deserialize, Default)]
struct Workspace {
    #[serde(default)]
    lints: BTreeMap<String, BTreeMap<String, toml::Value>>,
}

/// `(key, expected value, why it is load-bearing)`.
const RELEASE_PROFILE: [(&str, &str, &str); 3] = [
    (
        "panic",
        "unwind",
        "rule isolation is built on catch_unwind, which is a no-op under panic = \"abort\"",
    ),
    ("lto", "thin", "ADR-0001 pins the release profile"),
    ("codegen-units", "1", "ADR-0001 pins the release profile"),
];

const DELEGATED_CLIPPY: [(&str, &str); 2] = [
    ("too_many_lines", "the 60-code-line function gate"),
    ("too_many_arguments", "the 5-parameter gate"),
];

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let path = ctx.root.join("Cargo.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let root: Root = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut out = release_profile(&root);
    out.extend(delegated_clippy(&root));
    out.extend(clippy_thresholds(ctx)?);
    Ok(out)
}

fn release_profile(root: &Root) -> Vec<Violation> {
    let mut out = Vec::new();
    let release = root.profile.get("release");
    for (key, want, why) in RELEASE_PROFILE {
        let got = release.and_then(|r| r.get(key)).map(render);
        if got.as_deref() != Some(want) {
            out.push(Violation::new(
                RULE,
                "Cargo.toml",
                format!(
                    "[profile.release] {key} is {}, must be `{want}` — {why}",
                    got.unwrap_or_else(|| "unset".to_owned())
                ),
            ));
        }
    }
    out
}

fn delegated_clippy(root: &Root) -> Vec<Violation> {
    let mut out = Vec::new();
    let clippy = root.workspace.lints.get("clippy");
    for (lint, what) in DELEGATED_CLIPPY {
        let got = clippy.and_then(|c| c.get(lint)).map(render);
        if got.as_deref() != Some("deny") {
            out.push(Violation::new(
                RULE,
                "Cargo.toml",
                format!(
                    "[workspace.lints.clippy] {lint} is {}, must be `deny` — it carries {what}",
                    got.unwrap_or_else(|| "unset".to_owned())
                ),
            ));
        }
    }
    // The gate this workspace deliberately does NOT have. A nursery lint that misfires and
    // moves between toolchain releases turns CI red on code nobody touched.
    if clippy.is_some_and(|c| c.contains_key("cognitive_complexity")) {
        out.push(Violation::new(
            RULE,
            "Cargo.toml",
            "clippy::cognitive_complexity is a nursery lint and is deliberately absent \
             (ADR-0002 § 7); remove it"
                .to_owned(),
        ));
    }
    out
}

fn clippy_thresholds(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let path = ctx.root.join("clippy.toml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let cfg: BTreeMap<String, toml::Value> =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let mut out = Vec::new();
    for (key, want) in [("too-many-lines-threshold", "60"), ("too-many-arguments-threshold", "5")] {
        let got = cfg.get(key).map(render);
        if got.as_deref() != Some(want) {
            out.push(Violation::new(
                RULE,
                "clippy.toml",
                format!("{key} is {}, must be {want}", got.unwrap_or_else(|| "unset".into())),
            ));
        }
    }
    Ok(out)
}

/// A TOML scalar as the bare text the audit compares against.
fn render(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
