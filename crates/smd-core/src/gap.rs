//! `ResolutionGap` — why a resolution could not name a winner.

/// One per input the verdict depended on and could not establish. An empty set means the
/// outcome is stated. Copied onto the `Finding` for explanation text.
///
/// `ResolutionGap` is the specific reason; [`Certainty`](crate::Certainty) is its coarse
/// summary for policy and sorting. They are one source of truth in two resolutions.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[non_exhaustive]
pub enum ResolutionGap {
    /// A needed `rank` comparison hit an empty rank.
    OrderUnknown,
    /// An `Activation::Unknown` contender could outrank the winner.
    ActivationUnknown,
    /// The directory's cross-file behaviour is `MergeMode::Unknown`.
    MergeModeUnknown,
    /// The directory's Stage A behaviour is `SamePath::Unknown`.
    SamePathUnknown,
    /// Basename vs directory-relative path changes the winner.
    FilenameKeyForm,
    /// Whether vanilla joins the filename axis changes the winner.
    VanillaFilenameAxis,
    /// Vanilla-only vs vanilla-and-earlier `replace_path` scope changes the outcome.
    ReplacePathScope,
    /// First vs last duplicate in one file changes the winner.
    InFileOrder,
    /// Additive-body path without `AdditiveSplit` (Invariant A1).
    ChildDigestsUnavailable,
    /// A file in the subject's scope is not `Complete`.
    ParseIncomplete,
    /// One source ships two entries normalising to one path.
    IdentityCollision,
    /// Two paths tie on tier, settled file key, and rank.
    FileKeyTie,
}

impl ResolutionGap {
    /// The `snake_case` tag a `weights.toml` modifier compares against.
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            ResolutionGap::OrderUnknown => "order_unknown",
            ResolutionGap::ActivationUnknown => "activation_unknown",
            ResolutionGap::MergeModeUnknown => "merge_mode_unknown",
            ResolutionGap::SamePathUnknown => "same_path_unknown",
            ResolutionGap::FilenameKeyForm => "filename_key_form",
            ResolutionGap::VanillaFilenameAxis => "vanilla_filename_axis",
            ResolutionGap::ReplacePathScope => "replace_path_scope",
            ResolutionGap::InFileOrder => "in_file_order",
            ResolutionGap::ChildDigestsUnavailable => "child_digests_unavailable",
            ResolutionGap::ParseIncomplete => "parse_incomplete",
            ResolutionGap::IdentityCollision => "identity_collision",
            ResolutionGap::FileKeyTie => "file_key_tie",
        }
    }
}
