//! The explanation catalogue: `explain/<lang>.toml`, keyed by rule id.

use std::collections::BTreeMap;

use serde::Deserialize;
use smd_core::{Certainty, ResolvedEvidence, ResolvedFinding, Role};

use crate::PolicyError;
use crate::load::NON_BEST;

/// The prose for one finding, placeholders filled.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Explanation {
    /// One line.
    pub title: String,
    /// What happens.
    pub body: Option<String>,
    /// Why it matters.
    pub why: Option<String>,
    /// What the user can do.
    pub action: Option<String>,
    /// One sentence per non-best certainty axis. Never omitted by a renderer.
    pub qualifiers: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Text {
    title: String,
    body: Option<String>,
    why: Option<String>,
    action: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct RuleEntry {
    title: Option<String>,
    body: Option<String>,
    why: Option<String>,
    action: Option<String>,
    /// `variant.<evidence key>.<tag>`.
    #[serde(default)]
    variant: BTreeMap<String, BTreeMap<String, Text>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Qualifier {
    text: String,
}

#[derive(Deserialize)]
struct RawCatalogue {
    certainty: BTreeMap<String, BTreeMap<String, Qualifier>>,
    #[serde(flatten)]
    rules: BTreeMap<String, RuleEntry>,
}

/// A parsed catalogue.
#[derive(Clone, Debug)]
pub(crate) struct Catalogue {
    rules: BTreeMap<String, RuleEntry>,
    qualifiers: BTreeMap<String, BTreeMap<String, Qualifier>>,
}

pub(crate) fn catalogue(text: &str) -> Result<Catalogue, PolicyError> {
    let raw: RawCatalogue = toml::from_str(text).map_err(|e| PolicyError::Syntax(e.to_string()))?;
    for (axis, value) in NON_BEST {
        if raw.certainty.get(axis).and_then(|m| m.get(value)).is_none() {
            let why = format!(
                "explain: [certainty.{axis}.{value}] is missing; the qualifier is required"
            );
            return Err(PolicyError::Invalid(why));
        }
    }
    Ok(Catalogue { rules: raw.rules, qualifiers: raw.certainty })
}

fn tag_of<'a>(f: &'a ResolvedFinding, key: &str) -> Option<&'a str> {
    // Only a `Tag` selects a variant; any other value, including one added to the
    // (non-exhaustive) enum later, falls back to the rule's default text by design.
    f.facts
        .iter()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| if let ResolvedEvidence::Tag(t) = v { Some(*t) } else { None })
}

impl Catalogue {
    /// The first matching variant whose placeholders all fill, else the rule's default;
    /// `None` when the catalogue has no entry for the rule.
    pub(crate) fn explain(&self, f: &ResolvedFinding) -> Option<Explanation> {
        let entry = self.rules.get(f.rule.as_str())?;
        let variants = entry.variant.iter().filter_map(|(key, tags)| tags.get(tag_of(f, key)?));
        let default = entry.title.clone().map(|title| Text {
            title,
            body: entry.body.clone(),
            why: entry.why.clone(),
            action: entry.action.clone(),
        });
        let text = variants.chain(default.as_ref()).find_map(|t| render(t, f))?;
        Some(Explanation { qualifiers: self.qualifiers_for(f.certainty), ..text })
    }

    fn qualifiers_for(&self, c: Certainty) -> Vec<String> {
        let axes = [
            ("completeness", c.completeness.as_tag()),
            ("activation", c.activation.as_tag()),
            ("order", c.order.as_tag()),
            ("semantics", c.semantics.as_tag()),
        ];
        axes.iter()
            .filter_map(|(a, v)| self.qualifiers.get(*a).and_then(|m| m.get(*v)))
            .map(|q| q.text.clone())
            .collect()
    }
}

fn render(t: &Text, f: &ResolvedFinding) -> Option<Explanation> {
    // `Some(None)`: the field is absent. `None`: present but a placeholder did not fill.
    let fill_opt = |s: &Option<String>| match s.as_deref() {
        None => Some(None),
        Some(t) => fill(t, f).map(Some),
    };
    Some(Explanation {
        title: fill(&t.title, f)?,
        body: fill_opt(&t.body)?,
        why: fill_opt(&t.why)?,
        action: fill_opt(&t.action)?,
        qualifiers: Vec::new(),
    })
}

/// Fill every `{name}`; `None` when one cannot be filled (the caller falls back).
fn fill(text: &str, f: &ResolvedFinding) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}')?;
        out.push_str(&placeholder(&after[..close], f)?);
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Some(out)
}

fn names(f: &ResolvedFinding, role: Role) -> Vec<&str> {
    f.participants.iter().filter(|p| p.role == role).map(|p| &*p.mod_name).collect()
}

/// `a, b, c and N others`: truncated at three.
fn join(list: &[&str]) -> String {
    match list.len() {
        0..=3 => list.join(", "),
        n => format!("{} and {} others", list[..3].join(", "), n - 3),
    }
}

fn placeholder(name: &str, f: &ResolvedFinding) -> Option<String> {
    match name {
        "n" => Some(f.participants.len().to_string()),
        "subject" => Some(f.subject_text.to_string()),
        "winner" => match names(f, Role::Winner).as_slice() {
            [one] => Some((*one).to_owned()),
            _ => None,
        },
        "losers" => Some(join(&names(f, Role::Shadowed))).filter(|s| !s.is_empty()),
        "loser_first" => names(f, Role::Shadowed).first().map(|s| (*s).to_owned()),
        key => f.facts.iter().find(|(k, _)| *k == key).map(|(_, v)| match v {
            ResolvedEvidence::Int(i) => i.to_string(),
            ResolvedEvidence::Bool(b) => b.to_string(),
            ResolvedEvidence::Tag(t) => (*t).to_owned(),
            ResolvedEvidence::Text(t) => t.to_string(),
            _ => "<unhandled evidence>".to_owned(),
        }),
    }
}
