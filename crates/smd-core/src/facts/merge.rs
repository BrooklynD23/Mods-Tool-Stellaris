//! Merge classification: the two axes of a directory's behaviour.

/// How a directory's *differently named* files combine across mods — the cross-file axis,
/// and only that axis.
///
/// Paradox directories do not all behave alike, and treating them alike is why naive
/// conflict tools drown users in noise. But a directory has two axes: *same path* (two
/// sources ship the identical root-relative path, decided by the overlay before any key is
/// looked at, see [`SamePath`]) and *cross file* (this enum). Two mods each shipping the
/// same filename is a hard conflict in any directory; two mods shipping differently named
/// files in a [`MergeByKey`](MergeMode::MergeByKey) directory is usually fine. The
/// distinction comes from the game profile, not from rule code.
///
/// `PathOnly` was `FileReplace` in the M0 skeleton; it was renamed because same-path
/// replacement is no longer a mode (REVIEW-2026-10, H1, H7).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum MergeMode {
    /// No key layer. Files resolve by path identity alone; differently named files never
    /// interact. Binary assets.
    PathOnly,
    /// Across surviving files, definitions are grouped by key; the highest-precedence
    /// definition of a key wins and other keys from the losing file survive.
    MergeByKey,
    /// Merged by key, but the children named in `additive_children` accumulate from every
    /// participant. Gated by Invariant A1: resolvable only where the extractor emits
    /// [`AdditiveSplit`](crate::FactPayload::AdditiveSplit).
    ///
    /// `common/on_actions/` is what it exists for: every mod hooking `on_game_start`
    /// contributes effects and all of them run. Without this mode that is one
    /// 200-participant collision, and entirely false. (REVIEW-2026-08 § G4.)
    MergeByKeyAdditiveBody,
    /// Entries accumulate; duplicates coexist rather than override.
    Additive,
    /// The directory is read as exactly one logical file.
    Single,
    /// The profile does not classify this path's cross-file behaviour. No key-level
    /// outcome may be asserted for it.
    ///
    /// The default, deliberately: defaulting to [`MergeByKey`](MergeMode::MergeByKey)
    /// would produce confident verdicts about directories nobody has modelled.
    #[default]
    Unknown,
}

impl MergeMode {
    /// The `snake_case` name a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            MergeMode::PathOnly => "path_only",
            MergeMode::MergeByKey => "merge_by_key",
            MergeMode::MergeByKeyAdditiveBody => "merge_by_key_additive_body",
            MergeMode::Additive => "additive",
            MergeMode::Single => "single",
            MergeMode::Unknown => "unknown",
        }
    }
}

/// Whether Stage A's whole-file replacement applies on a path. Per directory, defaulted
/// from `[load_order].same_filename_replaces_whole_file`.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum SamePath {
    /// The winner's file replaces the loser's wholesale.
    Replace,
    /// The profile has not established it; resolves with
    /// [`ResolutionGap::SamePathUnknown`](crate::ResolutionGap::SamePathUnknown).
    #[default]
    Unknown,
}

impl SamePath {
    /// The `snake_case` tag.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            SamePath::Replace => "replace",
            SamePath::Unknown => "unknown",
        }
    }
}
