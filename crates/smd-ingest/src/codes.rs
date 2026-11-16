//! Diagnostic codes `smd-ingest` emits, and the sited-by-path diagnostic they travel in.

use smd_core::DiagSeverity;

/// A descriptor is over a cap. The mod is inventoried under its directory name with
/// content unread.
pub const DESCRIPTOR_INVALID: &str = "ingest.descriptor_invalid";
/// A descriptor is malformed below the caps; fields read before the fault are kept.
pub const DESCRIPTOR_SYNTAX: &str = "ingest.descriptor_syntax";
/// A descriptor file or directory could not be read at all.
pub const DESCRIPTOR_UNREADABLE: &str = "ingest.descriptor_unreadable";
/// `path=` or `archive=` resolved to a root, the home directory, or an ancestor of a root.
pub const ROOT_REFUSED: &str = "ingest.root_refused";
/// `path=` names a directory that does not exist.
pub const MOD_PATH_MISSING: &str = "ingest.mod_path_missing";
/// Two mods derived the same [`ModKey`](smd_core::ModKey); the later one was re-keyed.
pub const DUPLICATE_KEY: &str = "ingest.duplicate_mod_key";
/// A load-order source named a mod that discovery did not find.
pub const PLAYSET_MOD_UNMATCHED: &str = "ingest.playset_mod_unmatched";
/// A load-order source failed and the chain moved on.
pub const ORDER_SOURCE_FAILED: &str = "ingest.order_source_failed";
/// No load-order source answered.
pub const NO_ORDER_SOURCE: &str = "ingest.no_order_source";
/// The launcher database and `dlc_load.json` enable different sets of mods.
pub const ACTIVATION_DISAGREEMENT: &str = "ingest.activation_disagreement";
/// `dlc_load.json` is over a cap or not shaped as expected.
pub const DLC_LOAD_INVALID: &str = "ingest.dlc_load_invalid";
/// The launcher snapshot could not be verified; the chain fell through.
pub const LAUNCHER_SNAPSHOT_UNVERIFIED: &str = "ingest.launcher_snapshot_unverified";
/// The launcher schema is not the one the reader knows.
pub const LAUNCHER_SCHEMA_UNKNOWN: &str = "ingest.launcher_schema_unknown";
/// More than one active, non-removed playset.
pub const LAUNCHER_MULTIPLE_ACTIVE: &str = "ingest.launcher_multiple_active";
/// No active, non-removed playset.
pub const LAUNCHER_NO_ACTIVE: &str = "ingest.launcher_no_active_playset";
/// The launcher kept committing while it was read; the order may be seconds stale.
pub const LAUNCHER_CHANGING: &str = "ingest.launcher_changing";

/// A given location (install, user directory, workshop root, `mod/`) is not there.
pub const LOCATION_MISSING: &str = "ingest.location_missing";
/// Discovery skipped or could not list something (a link, a special file, an unreadable
/// entry, a bound that tripped), so the inventory may be missing mods.
pub const DISCOVERY_INCOMPLETE: &str = "ingest.discovery_incomplete";

/// A diagnostic that has no `Site`: the subject is a path or a source, not a byte range.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct IngestDiagnostic {
    /// Stable code, one of the constants in this module.
    pub code: &'static str,
    /// How badly.
    pub severity: DiagSeverity,
    /// What it is about, as display text (empty when it is scan-wide).
    pub subject: Box<str>,
    /// Human-readable specifics.
    pub detail: Box<str>,
}

impl IngestDiagnostic {
    /// A warning about `subject`.
    #[must_use]
    pub fn warn(code: &'static str, subject: &str, detail: impl Into<Box<str>>) -> Self {
        IngestDiagnostic {
            code,
            severity: DiagSeverity::Warn,
            subject: subject.into(),
            detail: detail.into(),
        }
    }

    /// An error about `subject`.
    #[must_use]
    pub fn error(code: &'static str, subject: &str, detail: impl Into<Box<str>>) -> Self {
        IngestDiagnostic {
            code,
            severity: DiagSeverity::Error,
            subject: subject.into(),
            detail: detail.into(),
        }
    }
}

/// A profile named a load-order source this build does not have.
pub const UNKNOWN_ORDER_SOURCE: &str = "ingest.unknown_order_source";
