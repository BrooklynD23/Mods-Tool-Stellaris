//! Diagnostics — parse failures are data, not control flow.
//!
//! A scan returns `(Report, Vec<Diagnostic>)`. The GUI surfaces the count in a corner
//! badge; the CLI prints a summary line and the full list under `-v`. **A scan with
//! diagnostics is still a successful scan** — one broken mod must not blind the user to
//! the other 299.

use crate::files::Site;

/// One thing that went wrong, where it went wrong, and how badly.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    /// Where to look.
    pub site: Site,
    /// A stable, namespaced code, e.g. `clausewitz.unbalanced_brace`.
    pub code: &'static str,
    /// How badly.
    pub severity: DiagSeverity,
    /// Human-readable specifics.
    pub detail: Box<str>,
}

impl Diagnostic {
    /// The code `#[non_exhaustive]` degradation must report.
    ///
    /// A wildcard arm over a shared enum converts a compile error into silent data loss:
    /// `_ => {}` in a renderer means a new [`Subject`](crate::Subject) variant renders as
    /// nothing, in a tool whose entire value is not omitting things. The rule is that such
    /// an arm emits this diagnostic and never `{}` or `None`; `cargo xtask lint` fails the
    /// empty forms. Loud degradation is the point of the attribute. (ADR-0002 § 3.)
    pub const UNHANDLED: &'static str = "core.unhandled_variant";

    /// Build the unhandled-variant diagnostic for a wildcard arm over a shared enum.
    #[must_use]
    pub fn unhandled(site: Site, type_name: &str, discriminant: u32) -> Diagnostic {
        Diagnostic {
            site,
            code: Diagnostic::UNHANDLED,
            severity: DiagSeverity::Warn,
            detail: format!("unhandled {type_name} variant, discriminant {discriminant}")
                .into_boxed_str(),
        }
    }
}

/// How badly a diagnostic went wrong. Neither level aborts a scan.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum DiagSeverity {
    /// Something was skipped; the rest of the file was read.
    Warn,
    /// The file could not be read at all.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::Span;
    use crate::ids::{PathId, SourceId};

    #[test]
    fn unhandled_names_the_type_and_the_discriminant() {
        let site = Site { source: SourceId(1), path: PathId(2), span: Span::default() };
        let d = Diagnostic::unhandled(site, "Subject", 7);
        assert_eq!(d.code, Diagnostic::UNHANDLED);
        assert_eq!(d.severity, DiagSeverity::Warn);
        assert!(d.detail.contains("Subject"));
        assert!(d.detail.contains('7'));
    }
}
