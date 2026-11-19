//! The per-axis qualifier sentence (doc 09, "The qualifier"; H12): one sentence for every
//! certainty axis that is not at its best value. Renderers print every one; nothing can
//! omit it.
//!
//! ponytail: the sentences are English constants here until the explanation catalogue's
//! `[certainty.*]` table exists (smd-policy, M5); then this reads the catalogue instead.

use crate::view::CertaintyView;

/// The qualifier sentences for `c`, in axis order: completeness, activation, order,
/// semantics. Empty when every axis is at its best value.
pub(crate) fn sentences(c: &CertaintyView) -> Vec<&'static str> {
    [completeness(c.completeness), activation(c.activation), order(c.order), semantics(c.semantics)]
        .into_iter()
        .flatten()
        .collect()
}

// Matched on the tag text, so a new enum value falls to the cautious generic sentence
// rather than to silence.
fn completeness(tag: &str) -> Option<&'static str> {
    match tag {
        "complete" => None,
        "partial" => {
            Some("Some files involved were only partly read; this finding may be incomplete.")
        }
        "unparsed" => Some("Some files involved could not be read; what they hold is unknown."),
        _ => Some("How completely the files were read is not established."),
    }
}

fn activation(tag: &str) -> Option<&'static str> {
    match tag {
        "certain" => None,
        "inferred" => Some("Which mods are enabled was inferred, not read from the launcher."),
        "conditional" => {
            Some("Would collide only if both mods are enabled; the launcher could not be read.")
        }
        _ => Some("Whether these mods are enabled is not established."),
    }
}

fn order(tag: &str) -> Option<&'static str> {
    match tag {
        "exact" => None,
        "inferred" => {
            Some("Load order was inferred from a secondary source; which mod wins may differ.")
        }
        "fallback" => Some("Load order unknown: which mod wins is not stated."),
        _ => Some("Load order is not established."),
    }
}

fn semantics(tag: &str) -> Option<&'static str> {
    match tag {
        "verified" => None,
        "assumed" => Some("The game behaviour this rests on is assumed, not yet verified in game."),
        "unmodelled" => {
            Some("The game behaviour here is not modelled; treat this as a lead, not a verdict.")
        }
        _ => Some("The game behaviour this rests on is not established."),
    }
}
