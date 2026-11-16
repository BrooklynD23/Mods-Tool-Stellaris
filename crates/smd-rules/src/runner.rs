//! Running rules: sorted by id, each under `catch_unwind` (doc 08 § Execution).

use std::any::Any;
use std::panic::catch_unwind;

use smd_core::{
    DiagSeverity, Diagnostic, Finding, PathId, Role, RuleId, Site, SourceId, Span, Subject,
};
use smd_index::ScanIndex;

use crate::rule::{Rule, RuleCtx};

/// Diagnostic code for a rule that panicked.
pub const RULE_PANICKED: &str = "rules.rule_panicked";

/// A rule that panicked, with its message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RulePanic {
    /// Which rule.
    pub rule: RuleId,
    /// The panic payload, when it was text.
    pub message: String,
}

impl RulePanic {
    fn new(rule: RuleId, payload: &(dyn Any + Send)) -> RulePanic {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "non-text panic payload".to_owned());
        RulePanic { rule, message }
    }

    /// The diagnostic a scan reports for it. It has no file, so the site is a placeholder.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            site: Site { source: SourceId::VANILLA, path: PathId(0), span: Span::default() },
            code: RULE_PANICKED,
            severity: DiagSeverity::Error,
            detail: format!("rule {} panicked: {}", self.rule, self.message).into_boxed_str(),
        }
    }
}

/// What a run produced.
#[derive(Debug, Default)]
pub struct RunOutput {
    /// Findings, concatenated in rule-id order.
    pub findings: Vec<Finding>,
    /// One per panicking rule.
    pub diagnostics: Vec<Diagnostic>,
    /// Cancellation was observed, so `findings` may be partial.
    pub cancelled: bool,
}

/// The contract checks behind the debug assertion: declared evidence keys of the declared
/// type, an unscored finding, the right rule id, at least one site, and invariant R1 (a
/// `Shadowed` participant on a path subject requires a `SingleWinner` or `Deleted` outcome).
///
/// # Errors
/// A description of the first violation.
pub fn check_finding(rule: &dyn Rule, index: &ScanIndex, f: &Finding) -> Result<(), String> {
    if f.rule != rule.id() {
        return Err(format!("{} emitted a finding as {}", rule.id(), f.rule));
    }
    if !f.is_unscored() {
        return Err(format!("{} set score or severity", rule.id()));
    }
    if f.participants.is_empty() {
        return Err(format!("{} emitted a finding with no site", rule.id()));
    }
    if let Subject::Path(p) = f.subject {
        let shadowed = f.participants.iter().any(|x| x.role == Role::Shadowed);
        if shadowed && !index.resolve_path(p).outcome.permits_shadowed_participants() {
            return Err(format!("{} broke R1: a Shadowed participant without a winner", rule.id()));
        }
    }
    for (key, value) in &f.facts {
        match rule.evidence().iter().find(|(k, _)| k == key) {
            None => return Err(format!("{} emitted undeclared evidence key {key}", rule.id())),
            Some((_, ty)) if !ty.accepts(*value) => {
                return Err(format!("{} emitted {key} with the wrong type", rule.id()));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Run one rule. A panic becomes an `Err`; the value boundary makes that sound.
///
/// # Errors
/// [`RulePanic`] when the rule unwound.
pub fn run_one(rule: &dyn Rule, ctx: &RuleCtx<'_>) -> Result<Vec<Finding>, RulePanic> {
    let found =
        catch_unwind(|| rule.run(ctx)).map_err(|p| RulePanic::new(rule.id(), p.as_ref()))?;
    for f in &found {
        if let Err(why) = check_finding(rule, ctx.index, f) {
            debug_assert!(false, "{why}");
        }
    }
    Ok(found)
}

/// Run `rules` sequentially in id order, isolating each.
#[must_use]
pub fn run_all(rules: &[&dyn Rule], ctx: &RuleCtx<'_>) -> RunOutput {
    let mut sorted: Vec<&dyn Rule> = rules.to_vec();
    sorted.sort_by_key(|r| r.id());
    let mut out = RunOutput::default();
    for rule in sorted {
        if ctx.cancel.is_cancelled() {
            out.cancelled = true;
            break;
        }
        match run_one(rule, ctx) {
            Ok(f) => out.findings.extend(f),
            Err(p) => out.diagnostics.push(p.diagnostic()),
        }
        // A rule that stopped early on cancellation returns what it had.
        out.cancelled |= ctx.cancel.is_cancelled();
    }
    out
}
