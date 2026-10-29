//! Hand-parsed arguments. Small enough that a parser crate would be the larger dependency.

use std::path::PathBuf;

/// The usage text, printed with every argument error.
pub const USAGE: &str = "\
usage: smd scan --game-dir <path> --user-dir <path> [options]

options:
  --game-dir <path>        the installed game directory (required)
  --user-dir <path>        the user data directory (required)
  --workshop-dir <path>    a workshop content directory (repeatable)
  --format table|json|md   output format (default: table)
  --no-redact              do not redact the home directory and account name
  --max-participants <n>   participants printed per finding (default: 10)
  --max-findings <n>       findings printed in total (default: 200)
  -h, --help               print this text

exit codes:
  0  the scan finished (conflicts found or not)
  1  the scan could not run
  2  bad command line
  3  the scan finished but reported an Error-class diagnostic (see the report)";

/// An output format.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Format {
    /// Terminal table.
    Table,
    /// JSON.
    Json,
    /// Markdown.
    Markdown,
}

/// The arguments of `smd scan`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanArgs {
    /// `--game-dir`.
    pub game_dir: PathBuf,
    /// `--user-dir`.
    pub user_dir: PathBuf,
    /// `--workshop-dir`, once per occurrence.
    pub workshop_dirs: Vec<PathBuf>,
    /// `--format`.
    pub format: Format,
    /// `--no-redact` turns this off.
    pub redact: bool,
    /// `--max-participants`.
    pub max_participants: Option<usize>,
    /// `--max-findings`.
    pub max_findings: Option<usize>,
}

/// What the command line asked for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Parsed {
    /// `smd scan ...`.
    Scan(ScanArgs),
    /// `--help`.
    Help,
}

fn value(it: &mut std::slice::Iter<'_, String>, flag: &str) -> Result<String, String> {
    it.next().cloned().ok_or_else(|| format!("{flag} needs a value"))
}

fn count(text: &str, flag: &str) -> Result<usize, String> {
    text.parse().map_err(|_| format!("{flag} needs a whole number, not `{text}`"))
}

fn format(text: &str) -> Result<Format, String> {
    match text {
        "table" => Ok(Format::Table),
        "json" => Ok(Format::Json),
        "md" | "markdown" => Ok(Format::Markdown),
        other => Err(format!("--format must be table, json or md, not `{other}`")),
    }
}

/// Parse everything after the program name.
///
/// # Errors
/// A message naming the first problem: an unknown flag or subcommand, a missing value, or
/// a missing required flag.
pub fn parse(argv: &[String]) -> Result<Parsed, String> {
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("-h" | "--help") => return Ok(Parsed::Help),
        Some("scan") => {}
        Some(other) => return Err(format!("unknown command `{other}`")),
        None => return Err("a command is required".to_owned()),
    }
    let mut a = ScanArgs {
        game_dir: PathBuf::new(),
        user_dir: PathBuf::new(),
        workshop_dirs: Vec::new(),
        format: Format::Table,
        redact: true,
        max_participants: None,
        max_findings: None,
    };
    let (mut game, mut user) = (false, false);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--game-dir" => (a.game_dir, game) = (value(&mut it, flag)?.into(), true),
            "--user-dir" => (a.user_dir, user) = (value(&mut it, flag)?.into(), true),
            "--workshop-dir" => a.workshop_dirs.push(value(&mut it, flag)?.into()),
            "--format" => a.format = format(&value(&mut it, flag)?)?,
            "--no-redact" => a.redact = false,
            "--max-participants" => a.max_participants = Some(count(&value(&mut it, flag)?, flag)?),
            "--max-findings" => a.max_findings = Some(count(&value(&mut it, flag)?, flag)?),
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    if !game {
        return Err("--game-dir is required".to_owned());
    }
    if !user {
        return Err("--user-dir is required".to_owned());
    }
    Ok(Parsed::Scan(a))
}
