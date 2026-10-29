//! The `dep-direction` gate.
//!
//! Reads the layer assignment from `xtask/layers.toml` — machine-readable, never
//! hand-drawn — and validates it against `cargo metadata`. Four checks:
//!
//! 1. every workspace member appears in `layers.toml`, and every entry is a member;
//! 2. every actual workspace-internal edge is declared in that crate's `deps`;
//! 3. every declared **and** every actual edge points to a strictly lower layer;
//! 4. a crate may not declare itself as a dependency.
//!
//! Cargo rejects an outright cycle before this gate ever runs. What this catches is the
//! edge that compiles and still inverts the architecture — `smd-report` reaching into
//! `smd-index`, or `smd-profiles` naming a load-order implementation.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Ctx, Violation};

mod metadata;

#[derive(Deserialize)]
struct LayerFile {
    crates: BTreeMap<String, CrateSpec>,
}

#[derive(Deserialize)]
pub struct CrateSpec {
    layer: u32,
    #[serde(default)]
    deps: Vec<String>,
    #[serde(default)]
    game_names_allowed: bool,
}

/// The layer map, loaded and keyed by crate name.
pub struct LayerMap {
    crates: BTreeMap<String, CrateSpec>,
}

impl LayerMap {
    pub fn load(ctx: &Ctx) -> Result<LayerMap, String> {
        let path = ctx.root.join("xtask/layers.toml");
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let parsed: LayerFile =
            toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(LayerMap { crates: parsed.crates })
    }

    pub fn layer_of(&self, name: &str) -> Option<u32> {
        self.crates.get(name).map(|c| c.layer)
    }

    pub fn game_names_allowed(&self, name: &str) -> bool {
        self.crates.get(name).is_some_and(|c| c.game_names_allowed)
    }
}

pub fn check(ctx: &Ctx) -> Result<Vec<Violation>, String> {
    let map = LayerMap::load(ctx)?;
    let members = metadata::workspace_members(&ctx.root)?;
    let mut out = coverage(&map, &members);
    out.extend(declared_edges(&map));
    out.extend(actual_edges(&map, &members));
    Ok(out)
}

/// Check 1 — the layer map names every crate, and only real crates.
fn coverage(map: &LayerMap, members: &[metadata::Member]) -> Vec<Violation> {
    let mut out = Vec::new();
    for m in members {
        if map.layer_of(&m.name).is_none() {
            out.push(Violation::new(
                "dep-direction",
                "xtask/layers.toml",
                format!(
                    "workspace member `{}` has no layer. A graph that omits a crate is \
                     worse than no graph: the lint permits every edge it cannot see",
                    m.name
                ),
            ));
        }
    }
    let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
    for name in map.crates.keys() {
        if !names.contains(&name.as_str()) {
            out.push(Violation::new(
                "dep-direction",
                "xtask/layers.toml",
                format!("`{name}` is not a workspace member"),
            ));
        }
    }
    out
}

/// Check 3 and 4, over the declared graph — so `layers.toml` cannot itself encode an
/// upward edge and quietly bless one.
fn declared_edges(map: &LayerMap) -> Vec<Violation> {
    let mut out = Vec::new();
    for (name, spec) in &map.crates {
        for dep in &spec.deps {
            if dep == name {
                out.push(Violation::new(
                    "dep-direction",
                    "xtask/layers.toml",
                    format!("`{name}` declares itself as a dependency"),
                ));
                continue;
            }
            let Some(dep_layer) = map.layer_of(dep) else {
                out.push(Violation::new(
                    "dep-direction",
                    "xtask/layers.toml",
                    format!("`{name}` declares `{dep}`, which has no layer"),
                ));
                continue;
            };
            if dep_layer >= spec.layer {
                out.push(Violation::new(
                    "dep-direction",
                    "xtask/layers.toml",
                    format!(
                        "declared edge `{name}` (layer {}) -> `{dep}` (layer {dep_layer}) \
                         does not point downward",
                        spec.layer
                    ),
                ));
            }
        }
    }
    out
}

/// Checks 2 and 3, over the graph `cargo metadata` actually reports.
fn actual_edges(map: &LayerMap, members: &[metadata::Member]) -> Vec<Violation> {
    let mut out = Vec::new();
    let member_names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();

    for m in members {
        let Some(spec) = map.crates.get(&m.name) else { continue };
        for dep in &m.deps {
            if !member_names.contains(&dep.as_str()) {
                continue;
            }
            let Some(dep_layer) = map.layer_of(dep) else { continue };
            if dep_layer >= spec.layer {
                out.push(Violation::new(
                    "dep-direction",
                    format!("crates/{}/Cargo.toml", m.name),
                    format!(
                        "UPWARD DEPENDENCY: `{}` (layer {}) depends on `{dep}` (layer \
                         {dep_layer}). Dependencies point downward only",
                        m.name, spec.layer
                    ),
                ));
            } else if !spec.deps.contains(dep) {
                out.push(Violation::new(
                    "dep-direction",
                    format!("crates/{}/Cargo.toml", m.name),
                    format!(
                        "undeclared edge `{}` -> `{dep}`: add it to xtask/layers.toml or \
                         remove the dependency",
                        m.name
                    ),
                ));
            }
        }
    }
    out
}
