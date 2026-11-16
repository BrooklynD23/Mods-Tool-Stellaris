//! The `cargo metadata` half of the dependency gate.
//!
//! The graph is read from cargo rather than from the manifests directly, so that path
//! deps, renamed deps, dev-deps, and build-deps all arrive already resolved and the gate
//! cannot be fooled by a manifest shape it did not anticipate.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
}

/// One workspace member and the workspace-internal crates it depends on.
pub struct Member {
    pub name: String,
    /// Every dependency name, of every kind. Filtering to workspace members is the
    /// caller's job, because it also owns the list of members.
    pub deps: Vec<String>,
}

pub fn workspace_members(root: &Path) -> Result<Vec<Member>, String> {
    let out = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["metadata", "--format-version", "1", "--no-deps", "--all-features"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("running `cargo metadata`: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let meta: Metadata =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("parsing cargo metadata: {e}"))?;

    let mut members: Vec<Member> = meta
        .packages
        .into_iter()
        .filter(|p| meta.workspace_members.contains(&p.id))
        .map(|p| {
            let mut deps: Vec<String> = p.dependencies.into_iter().map(|d| d.name).collect();
            deps.sort();
            deps.dedup();
            Member { name: p.name, deps }
        })
        .collect();
    members.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(members)
}
