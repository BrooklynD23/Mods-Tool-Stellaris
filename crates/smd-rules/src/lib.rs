//! `smd-rules` — The rule engine and one file per rule.
//!
//! **Owns:** `Rule` trait, engine, one rule per file, per-rule `evidence()` and `cases()`.
//!
//! **Must never:** Format output, hardcode a score, or name a game.
//!
//! # Registration, for now
//!
//! Doc 08 specifies `register_rule!` over `inventory`, a const assertion that a rule's
//! fixtures cover every [`CaseKind`], and a `--selftest` diff against `registry.lock`
//! (H36). None of that exists yet. Until it does, [`registry`] is an explicit function
//! returning the rules sorted by id, and each rule file holds its own
//! `const _: () = RuleCase::assert_covers_all_kinds(..)`, so the compile-time half of the
//! coverage gate is already live. A new rule is one file plus one line in [`registry`].
//!
//! The fan-out over rules belongs to `smd-engine`; [`run_all`] is deliberately
//! sequential.

#![forbid(unsafe_code)]

mod case;
mod registry;
pub mod rule;
pub mod rules;
mod runner;

#[cfg(test)]
mod tests;

pub use case::{CaseKind, Expectation, RuleCase, WinnerExpect, run_case};
pub use registry::registry;
pub use rule::{CancellationToken, Rule, RuleCtx};
pub use runner::{RULE_PANICKED, RulePanic, RunOutput, check_finding, run_all, run_one};
