//! Hand-parsed arguments. Small enough that a parser crate would be the larger dependency.

use std::path::PathBuf;

/// The usage text, printed with every argument error.
pub const USAGE: &str = "\
usage: smd scan [options]
       smd locate [options]

smd scan reports same-path conflicts between the installed mods.
smd locate prints where the game, its user directory and every workshop directory
are on this machine, how each was found, and every mod found there.

options for both:
  --game-dir <path>        the installed game directory
  --user-dir <path>        the user data directory
  --workshop-dir <path>    a workshop content directory (repeatable)
  --format table|json|md   output format (default: table)
  --no-redact              do not redact the home directory and account name
  -h, --help               print this text

options for scan only:
  --max-participants <n>   participants printed per finding (default: 10)
  --max-findings <n>       findings printed in total (default: 200)
  --progress               print `[n/N] <mod>` to stderr per mod walked (on by default
                           when stderr is a terminal)

Any location not given is searched for: an SMD_GAME_DIR / SMD_USER_DIR environment
variable, then Steam libraries (SMD_STEAM_DIR, the registry, the usual folders), then
the registry and the OS defaults. `smd locate` shows every place tried. `smd scan`
searches unless both --game-dir and --user-dir are given; then only the
--workshop-dir roots given are read.

exit codes:
  0  finished (a scan that found conflicts, or none, exits 0)
  1  could not run: a given directory is unreadable, or the output could not be written
  2  bad command line, or a location `smd scan` needs could not be found
  3  finished but reported an Error-class diagnostic (see the report)";

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

/// Flags both commands take.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Common {
    /// `--game-dir`.
    pub game_dir: Option<PathBuf>,
    /// `--user-dir`.
    pub user_dir: Option<PathBuf>,
    /// `--workshop-dir`, once per occurrence.
    pub workshop_dirs: Vec<PathBuf>,
    /// `--format`.
    pub format: Format,
    /// `--no-redact` turns this off.
    pub redact: bool,
}

/// The arguments of `smd scan`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ScanArgs {
    /// The flags shared with `locate`.
    pub common: Common,
    /// `--max-participants`.
    pub max_participants: Option<usize>,
    /// `--max-findings`.
    pub max_findings: Option<usize>,
    /// `--progress`.
    pub progress: bool,
}

/// What the command line asked for.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Parsed {
    /// `smd scan ...`.
    Scan(ScanArgs),
    /// `smd locate ...`.
    Locate(Common),
    /// `--help`.
    Help,
}

type Args<'a> = std::slice::Iter<'a, String>;

fn value(it: &mut Args<'_>, flag: &str) -> Result<String, String> {
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

/// Take `flag` if it is one both commands share; `Ok(false)` when it is not.
fn shared(flag: &str, it: &mut Args<'_>, c: &mut Common) -> Result<bool, String> {
    match flag {
        "--game-dir" => c.game_dir = Some(value(it, flag)?.into()),
        "--user-dir" => c.user_dir = Some(value(it, flag)?.into()),
        "--workshop-dir" => c.workshop_dirs.push(value(it, flag)?.into()),
        "--format" => c.format = format(&value(it, flag)?)?,
        "--no-redact" => c.redact = false,
        _ => return Ok(false),
    }
    Ok(true)
}

fn scan(mut it: Args<'_>) -> Result<Parsed, String> {
    let mut a = ScanArgs {
        common: new_common(),
        max_participants: None,
        max_findings: None,
        progress: false,
    };
    while let Some(flag) = it.next() {
        if shared(flag, &mut it, &mut a.common)? {
            continue;
        }
        match flag.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            "--max-participants" => a.max_participants = Some(count(&value(&mut it, flag)?, flag)?),
            "--max-findings" => a.max_findings = Some(count(&value(&mut it, flag)?, flag)?),
            "--progress" => a.progress = true,
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Parsed::Scan(a))
}

fn locate(mut it: Args<'_>) -> Result<Parsed, String> {
    let mut c = new_common();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "-h" | "--help" => return Ok(Parsed::Help),
            _ if shared(flag, &mut it, &mut c)? => {}
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Parsed::Locate(c))
}

fn new_common() -> Common {
    Common {
        game_dir: None,
        user_dir: None,
        workshop_dirs: Vec::new(),
        format: Format::Table,
        redact: true,
    }
}

/// Parse everything after the program name.
///
/// # Errors
/// A message naming the first problem: an unknown flag or subcommand, or a missing or bad
/// value.
pub fn parse(argv: &[String]) -> Result<Parsed, String> {
    let mut it = argv.iter();
    match it.next().map(String::as_str) {
        Some("-h" | "--help") => Ok(Parsed::Help),
        Some("scan") => scan(it),
        Some("locate") => locate(it),
        Some(other) => Err(format!("unknown command `{other}`")),
        None => Err("a command is required".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Parsed, String> {
        parse(&args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn scan_paths_are_optional() {
        let Ok(Parsed::Scan(a)) = p(&["scan"]) else { panic!("scan parses") };
        assert_eq!((a.common.game_dir, a.common.user_dir), (None, None));
    }

    #[test]
    fn locate_takes_the_shared_flags_and_repeats_workshop_dirs() {
        let Ok(Parsed::Locate(c)) = p(&[
            "locate",
            "--format",
            "json",
            "--game-dir",
            "/g",
            "--workshop-dir",
            "/a",
            "--workshop-dir",
            "/b",
            "--no-redact",
        ]) else {
            panic!("locate parses")
        };
        assert_eq!(c.format, Format::Json);
        assert_eq!(c.workshop_dirs.len(), 2);
        assert!(!c.redact && c.game_dir.is_some() && c.user_dir.is_none());
    }

    #[test]
    fn scan_only_flags_are_rejected_by_locate() {
        assert!(p(&["locate", "--max-findings", "3"]).is_err());
        assert!(p(&["scan", "--max-findings", "x"]).is_err());
        assert!(p(&["scan", "--game-dir"]).is_err());
        assert_eq!(p(&["locate", "-h"]), Ok(Parsed::Help));
    }
}
