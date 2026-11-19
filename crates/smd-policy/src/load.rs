//! Reading `weights.toml`: base weights, modifiers, bands, certainty caps.

use std::collections::BTreeMap;

use serde::Deserialize;
use smd_core::Severity;

use crate::PolicyError;

/// Every non-best value of every certainty axis. Each must have a `[caps]` row and a
/// `[certainty.*]` qualifier: an absent cap is an uncertain finding at full volume.
pub(crate) const NON_BEST: [(&str, &str); 8] = [
    ("completeness", "partial"),
    ("completeness", "unparsed"),
    ("activation", "inferred"),
    ("activation", "conditional"),
    ("order", "inferred"),
    ("order", "fallback"),
    ("semantics", "assumed"),
    ("semantics", "unmodelled"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWeights {
    base: BTreeMap<String, i32>,
    #[serde(default)]
    modifier: Vec<Modifier>,
    bands: Bands,
    caps: BTreeMap<String, BTreeMap<String, String>>,
}

/// One `[[modifier]]`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Modifier {
    pub when: When,
    pub add: i32,
    /// Prose for the TOML's reader; accepted so `deny_unknown_fields` does not reject it.
    #[serde(default)]
    #[expect(dead_code, reason = "documentation in the data file; no code reads it yet")]
    pub why: Option<String>,
}

/// A modifier's condition. Every field present must hold.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(deny_unknown_fields)]
pub(crate) struct When {
    pub evidence: Option<String>,
    pub eq: Option<String>,
    pub path_glob: Option<Globs>,
    pub participants_gte: Option<usize>,
    pub rule: Option<String>,
    /// Phase 2: any participant mod has at least this many findings in the population.
    pub same_mod_findings_gte: Option<usize>,
}

/// One glob or several.
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub(crate) enum Globs {
    One(String),
    Many(Vec<String>),
}

impl Globs {
    pub(crate) fn list(&self) -> &[String] {
        match self {
            Globs::One(g) => std::slice::from_ref(g),
            Globs::Many(v) => v,
        }
    }
}

/// `[bands]`: the lowest score of each band.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bands {
    pub info: i32,
    pub low: i32,
    pub medium: i32,
    pub high: i32,
    pub critical: i32,
}

impl Bands {
    pub(crate) fn band(self, score: i32) -> Severity {
        match score {
            s if s >= self.critical => Severity::Critical,
            s if s >= self.high => Severity::High,
            s if s >= self.medium => Severity::Medium,
            s if s >= self.low => Severity::Low,
            _ => Severity::Info,
        }
    }
}

/// The parsed `weights.toml`.
#[derive(Clone, Debug)]
pub(crate) struct Weights {
    pub base: BTreeMap<String, i32>,
    pub modifiers: Vec<Modifier>,
    pub bands: Bands,
    /// `(axis, value)` to the highest band a finding with that value may reach.
    pub caps: BTreeMap<(String, String), Severity>,
}

pub(crate) fn severity(text: &str) -> Option<Severity> {
    Some(match text {
        "info" => Severity::Info,
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => return None,
    })
}

fn bad(what: impl Into<String>) -> PolicyError {
    PolicyError::Invalid(what.into())
}

fn check_modifier(m: &Modifier) -> Result<(), PolicyError> {
    if m.when.evidence.is_some() != m.when.eq.is_some() {
        return Err(bad("a modifier's `evidence` and `eq` must be given together"));
    }
    let w = &m.when;
    let empty = w.evidence.is_none()
        && w.path_glob.is_none()
        && w.participants_gte.is_none()
        && w.rule.is_none()
        && w.same_mod_findings_gte.is_none();
    if empty {
        return Err(bad("a modifier with an empty `when` would match every finding"));
    }
    Ok(())
}

fn caps(
    raw: BTreeMap<String, BTreeMap<String, String>>,
) -> Result<BTreeMap<(String, String), Severity>, PolicyError> {
    let mut out = BTreeMap::new();
    for (axis, values) in raw {
        for (value, band) in values {
            let known = NON_BEST.iter().any(|(a, v)| *a == axis && *v == value);
            if !known {
                return Err(bad(format!("caps.{axis}.{value}: no such non-best certainty value")));
            }
            let sev =
                severity(&band).ok_or_else(|| bad(format!("caps.{axis}.{value}: `{band}`")))?;
            out.insert((axis.clone(), value), sev);
        }
    }
    if let Some((a, v)) =
        NON_BEST.iter().find(|(a, v)| !out.contains_key(&((*a).to_owned(), (*v).to_owned())))
    {
        return Err(bad(format!("caps.{a}.{v} is missing; a cap may be lowered, never removed")));
    }
    Ok(out)
}

pub(crate) fn weights(text: &str) -> Result<Weights, PolicyError> {
    let raw: RawWeights = toml::from_str(text).map_err(|e| PolicyError::Syntax(e.to_string()))?;
    let b = raw.bands;
    if !(b.info <= b.low && b.low <= b.medium && b.medium <= b.high && b.high <= b.critical) {
        return Err(bad("[bands] must be ascending: info <= low <= medium <= high <= critical"));
    }
    raw.modifier.iter().try_for_each(check_modifier)?;
    Ok(Weights { base: raw.base, modifiers: raw.modifier, bands: b, caps: caps(raw.caps)? })
}
