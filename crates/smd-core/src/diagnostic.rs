//! Diagnostics — parse failures are data, not control flow.
//!
//! A scan returns `(Report, Vec<Diagnostic>)`. The GUI surfaces the count in a corner
//! badge; the CLI prints a summary line and the full list under `-v`. **A scan with
//! diagnostics is still a successful scan** — one broken mod must not blind the user to
//! the other 299.

use smallvec::SmallVec;

use crate::files::{Site, Span, UnparsedReason};
use crate::ids::ModKey;

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

/// File-local form of [`Diagnostic`], cacheable like [`RawFact`](crate::RawFact) and sited
/// at rehydration.
///
/// Cached beside the facts and the file's `ParseCompleteness`, so a warm scan replays the
/// cold scan's recovery diagnostics instead of reading a `Partial` file as clean.
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawDiagnostic {
    /// Raw-file byte offsets within the file.
    pub span: Span,
    /// Resolved against the code registry on decode.
    pub code: Box<str>,
    /// How badly.
    pub severity: DiagSeverity,
    /// Human-readable specifics.
    pub detail: Box<str>,
}

/// Scan-wide summary of what was not fully read. Printed by every report surface, even
/// with zero findings. Text and stable keys only, so it crosses the API boundary as-is
/// (Invariant C1).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScanCompleteness {
    /// Every file enumerated.
    pub files_total: u32,
    /// Files with `ParseCompleteness::Partial`.
    pub partial: u32,
    /// Unparsed files by reason; zero counts omitted.
    pub unparsed: SmallVec<[(UnparsedReason, u32); 5]>,
    /// Domain names holding at least one `Partial` or `Unparsed` file, sorted.
    pub domains: Vec<Box<str>>,
    /// Sources whose tree exceeded an enumeration bound, or whose archive was refused or
    /// not read: content-incomplete for any key and any directory. The text is the
    /// mod-root-relative path where reading stopped (the refused link, the bound that was
    /// hit), when there is one.
    pub content_incomplete: Vec<(ModKey, ContentIncompleteReason, Option<Box<str>>)>,
}

/// Why a source's content is incomplete.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ContentIncompleteReason {
    /// The tree exceeded an enumeration bound.
    Truncated,
    /// The archive was refused at the trust boundary.
    ArchiveRefused,
    /// The archive was not read.
    ArchiveUnread,
    /// A directory link was refused at the trust boundary (doc 05), so its subtree was
    /// never walked.
    LinkRefused,
}

/// How badly a diagnostic went wrong. Neither level aborts a scan.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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

    #[test]
    fn scan_completeness_carries_refusals_of_both_kinds() {
        let c = ScanCompleteness {
            files_total: 10,
            partial: 1,
            unparsed: SmallVec::from_slice(&[
                (UnparsedReason::Refused, 2),
                (UnparsedReason::Oversize, 1),
            ]),
            domains: vec!["trait".into()],
            content_incomplete: vec![(
                ModKey("m".into()),
                ContentIncompleteReason::LinkRefused,
                Some("common/link".into()),
            )],
        };
        assert_eq!(c.unparsed[0].0, UnparsedReason::Refused);
        assert_eq!(c.content_incomplete[0].1, ContentIncompleteReason::LinkRefused);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn scan_completeness_round_trips_through_serde() {
        let c = ScanCompleteness {
            files_total: 3,
            partial: 1,
            unparsed: SmallVec::from_slice(&[(UnparsedReason::NoParser, 2)]),
            domains: vec!["trait".into()],
            content_incomplete: vec![(
                ModKey("m".into()),
                ContentIncompleteReason::Truncated,
                None,
            )],
        };
        let json = serde_json::to_string(&c).expect("serialises");
        assert_eq!(serde_json::from_str::<ScanCompleteness>(&json).expect("deserialises"), c);
    }

    /// These are cached data and serde encodes variants by name or position; a reorder or
    /// rename must be a deliberate, versioned change.
    #[cfg(feature = "serde")]
    #[test]
    fn cached_enums_serialise_stably() {
        let all = [
            serde_json::to_string(&DiagSeverity::Warn).unwrap(),
            serde_json::to_string(&DiagSeverity::Error).unwrap(),
            serde_json::to_string(&UnparsedReason::Oversize).unwrap(),
            serde_json::to_string(&UnparsedReason::Unreadable).unwrap(),
            serde_json::to_string(&UnparsedReason::ChangedDuringScan).unwrap(),
            serde_json::to_string(&UnparsedReason::NoParser).unwrap(),
            serde_json::to_string(&UnparsedReason::Refused).unwrap(),
            serde_json::to_string(&crate::ParseCompleteness::Complete).unwrap(),
            serde_json::to_string(&crate::ParseCompleteness::Partial).unwrap(),
            serde_json::to_string(&crate::ParseCompleteness::Unparsed(UnparsedReason::Refused))
                .unwrap(),
            serde_json::to_string(&crate::ParseCompleteness::NotApplicable).unwrap(),
            serde_json::to_string(&crate::FileKind::Script).unwrap(),
            serde_json::to_string(&crate::FileKind::Unknown).unwrap(),
        ];
        let expected = [
            r#""Warn""#,
            r#""Error""#,
            r#""Oversize""#,
            r#""Unreadable""#,
            r#""ChangedDuringScan""#,
            r#""NoParser""#,
            r#""Refused""#,
            r#""Complete""#,
            r#""Partial""#,
            r#"{"Unparsed":"Refused"}"#,
            r#""NotApplicable""#,
            r#""Script""#,
            r#""Unknown""#,
        ];
        assert_eq!(all.as_slice(), expected.as_slice());
    }

    #[cfg(feature = "serde")]
    #[test]
    fn raw_diagnostic_round_trips_through_serde() {
        let d = RawDiagnostic {
            span: Span { offset: 1, len: 2, line: 3 },
            code: "clausewitz.unbalanced_brace".into(),
            severity: DiagSeverity::Warn,
            detail: "x".into(),
        };
        let json = serde_json::to_string(&d).expect("serialises");
        assert_eq!(serde_json::from_str::<RawDiagnostic>(&json).expect("deserialises"), d);
    }
}
