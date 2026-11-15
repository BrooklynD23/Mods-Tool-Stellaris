//! What a scan is asked to do, and why it can refuse.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use smd_fs::WalkLimits;
use smd_profiles::ProfileError;
use smd_rules::CancellationToken;

/// One source about to be walked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    /// 1-based position of this source among the sources that will be walked.
    pub index: usize,
    /// How many sources will be walked.
    pub total: usize,
    /// The source's key.
    pub key: String,
    /// The source's display name, when it has one.
    pub name: String,
}

/// A progress callback, called once per source before it is walked.
#[derive(Clone)]
pub struct ProgressFn(pub Arc<dyn Fn(Progress) + Send + Sync>);

impl fmt::Debug for ProgressFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProgressFn")
    }
}

/// One scan: where the install, the user directory and the workshop are, and its bounds.
#[derive(Clone, Debug)]
pub struct ScanRequest {
    /// The installed game tree. Vanilla is read from here and from nowhere else.
    pub game_dir: PathBuf,
    /// The user directory: the launcher files, `dlc_load.json`, local mods.
    pub user_dir: PathBuf,
    /// Workshop content roots. At most one is supported for now.
    pub workshop_dirs: Vec<PathBuf>,
    /// Bounds on each source's walk.
    pub limits: WalkLimits,
    /// Checked per source and per rule. Cancelling makes the scan return
    /// [`ScanError::Cancelled`]; a half-scan is never reported as a scan.
    pub cancel: CancellationToken,
    /// The home directory discovery must never treat as a mod root. `None` reads
    /// `HOME` / `USERPROFILE`.
    pub home: Option<PathBuf>,
    /// Called once per walked source, in walk order. Never affects the output.
    pub progress: Option<ProgressFn>,
}

impl ScanRequest {
    /// A request over a game directory and a user directory, with default bounds, no
    /// workshop and an unset cancellation token.
    #[must_use]
    pub fn new(game_dir: PathBuf, user_dir: PathBuf) -> ScanRequest {
        ScanRequest {
            game_dir,
            user_dir,
            workshop_dirs: Vec::new(),
            limits: WalkLimits::default(),
            cancel: CancellationToken::new(),
            home: None,
            progress: None,
        }
    }
}

/// Why a scan did not produce an output.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ScanError {
    /// A root the request names could not be opened as a directory.
    RootUnreadable {
        /// Which root: `game`, `user` or `workshop`.
        role: &'static str,
        /// The path given.
        path: PathBuf,
        /// The OS reason.
        reason: String,
    },
    /// The request asks for something not built yet.
    Unsupported(&'static str),
    /// The shipped profile did not read.
    Profile(ProfileError),
    /// The cancellation token fired.
    Cancelled,
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::RootUnreadable { role, path, reason } => {
                write!(f, "cannot read the {role} directory {}: {reason}", path.display())
            }
            ScanError::Unsupported(what) => write!(f, "not supported yet: {what}"),
            ScanError::Profile(e) => write!(f, "{e}"),
            ScanError::Cancelled => f.write_str("the scan was cancelled"),
        }
    }
}

impl std::error::Error for ScanError {}
