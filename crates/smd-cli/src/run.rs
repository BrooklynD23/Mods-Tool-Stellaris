//! Running a command line: parse, scan or locate, render, exit code.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::sync::Arc;

use smd_api::{
    Command, LocateOutput, LocateRequest, Outcome, Overrides, ProgressFn, ScanError, ScanRequest,
};
use smd_core::DiagSeverity;
use smd_report::{
    Caps, DiagnosticRow, DiagnosticsFormat, Json, Markdown, RedactionContext, RenderOptions,
    Renderer, Table, render_diagnostics, render_locate,
};

use crate::args::{self, Common, Format, Parsed, ScanArgs, USAGE};
use crate::model::{locate_report, report_model};

/// The process environment the CLI reads, passed in so tests need not touch the real one.
#[derive(Clone, Debug)]
pub struct Env {
    /// What redaction removes.
    pub redaction: RedactionContext,
    /// Whether stdout is a terminal; colour is on only then.
    pub stdout_is_tty: bool,
}

/// Exit code for a finished command.
const OK: i32 = 0;
/// Exit code for a command that finished but emitted an Error-class diagnostic (a file or
/// directory could not be read). Never used because conflicts were found.
const DIAGNOSTIC_ERROR: i32 = 3;
/// Exit code for a command that could not run.
const FAILED: i32 = 1;
/// Exit code for a bad command line, or a location `scan` needs that could not be found.
const USAGE_ERROR: i32 = 2;

/// Run `smd` with `argv` (the arguments after the program name). Returns the exit code.
pub fn run(argv: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match args::parse(argv) {
        Ok(Parsed::Help) => {
            let _ = writeln!(out, "{USAGE}");
            OK
        }
        Ok(Parsed::Scan(a)) => scan(&a, env, out, err),
        Ok(Parsed::Locate(c)) => locate(&c, env, out, err),
        Err(msg) => {
            let _ = writeln!(err, "smd: {msg}\n\n{USAGE}");
            USAGE_ERROR
        }
    }
}

fn fail(c: &Common, env: &Env, err: &mut dyn Write, code: &str, e: &ScanError) -> i32 {
    let row = DiagnosticRow {
        code: code.to_owned(),
        level: DiagSeverity::Error,
        location: String::new(),
        detail: e.to_string(),
    };
    let opts = options(c, env, Caps::default());
    let _ = write!(err, "{}", render_diagnostics(DiagnosticsFormat::Table, &[row], &opts));
    FAILED
}

fn emit(out: &mut dyn Write, err: &mut dyn Write, text: &str, had_error: bool) -> i32 {
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) if had_error => DIAGNOSTIC_ERROR,
        Ok(()) => OK,
        Err(e) => {
            let _ = writeln!(err, "smd: cannot write the report: {}", e.kind());
            FAILED
        }
    }
}

fn options(c: &Common, env: &Env, caps: Caps) -> RenderOptions {
    let mut opts =
        RenderOptions::new(env.redaction.clone()).with_color(env.stdout_is_tty).with_caps(caps);
    if !c.redact {
        opts = opts.without_redaction();
    }
    opts
}

fn locate_output(c: &Common) -> Result<LocateOutput, ScanError> {
    let overrides = Overrides {
        game_dir: c.game_dir.clone(),
        user_dir: c.user_dir.clone(),
        workshop_dirs: c.workshop_dirs.clone(),
    };
    match smd_api::execute(Command::Locate(LocateRequest {
        overrides,
        home: None,
        game: c.game.clone(),
    }))? {
        Outcome::Locate(o) => Ok(*o),
        Outcome::Scan(_) => Err(ScanError::Unsupported("a scan outcome for a locate command")),
    }
}

fn locate(c: &Common, env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let found = match locate_output(c) {
        Ok(o) => o,
        Err(e) => return fail(c, env, err, "smd.locate_failed", &e),
    };
    let had_error = found.diagnostics.iter().any(|d| d.level == DiagSeverity::Error);
    let format = match c.format {
        Format::Table => DiagnosticsFormat::Table,
        Format::Json => DiagnosticsFormat::Json,
        Format::Markdown => DiagnosticsFormat::Markdown,
    };
    let text = render_locate(format, &locate_report(found), &options(c, env, Caps::default()));
    emit(out, err, &text, had_error)
}

/// Game dir, user dir, workshop dirs, and whether locating them left a gap.
type Roots = (PathBuf, PathBuf, Vec<PathBuf>, bool);

/// The game, user and workshop directories a scan reads. Given both `--game-dir` and
/// `--user-dir`, nothing is searched for; otherwise whatever is missing is located, and a
/// location that cannot be found is exit 2 with every place tried.
fn resolve(c: &Common, env: &Env, err: &mut dyn Write) -> Result<Roots, i32> {
    check_given_dirs(c, env, err)?;
    if let (Some(g), Some(u)) = (&c.game_dir, &c.user_dir) {
        return Ok((g.clone(), u.clone(), c.workshop_dirs.clone(), false));
    }
    let found = locate_output(c).map_err(|e| fail(c, env, err, "smd.locate_failed", &e))?;
    let (game, user) = (found.located.game_dir.clone(), found.located.user_dir.clone());
    let workshops = if c.workshop_dirs.is_empty() {
        found.located.workshop_dirs.clone()
    } else {
        c.workshop_dirs.clone()
    };
    match (game, user) {
        (Some(g), Some(u)) => {
            // A mistyped override falls through to a search; say so instead of staying quiet.
            warn_rows(c, env, err, &diagnostic_rows(&found));
            Ok((g, u, workshops, found.incomplete))
        }
        _ => {
            let rows = unresolved_rows(&found);
            let opts = options(c, env, Caps::default());
            let text = render_diagnostics(DiagnosticsFormat::Table, &rows, &opts);
            let _ =
                write!(err, "smd: could not find every directory a scan needs:\n{text}\n{USAGE}\n");
            Err(USAGE_ERROR)
        }
    }
}

/// What was not found, and every place that was tried for it.
fn unresolved_rows(found: &LocateOutput) -> Vec<DiagnosticRow> {
    let l = &found.located;
    let missing = [("game_dir", l.game_dir.is_none()), ("user_dir", l.user_dir.is_none())];
    let mut rows = Vec::new();
    for key in missing.iter().filter(|(_, none)| *none).map(|(k, _)| *k) {
        let row = |code: &str, level, detail: String| DiagnosticRow {
            code: code.to_owned(),
            level,
            location: key.to_owned(),
            detail,
        };
        let hint = "not found; pass --game-dir / --user-dir, or set SMD_GAME_DIR / SMD_USER_DIR";
        rows.push(row("smd.location_not_found", DiagSeverity::Error, hint.to_owned()));
        for c in l.candidates.iter().filter(|c| c.key == key) {
            let how = if c.exists { "exists" } else { "absent" };
            let detail = format!("tried {} ({}, {how})", c.path.display(), c.origin.as_str());
            rows.push(row("smd.location_tried", DiagSeverity::Warn, detail));
        }
    }
    rows.extend(diagnostic_rows(found));
    rows
}

/// The locate diagnostics as printable rows.
fn diagnostic_rows(found: &LocateOutput) -> Vec<DiagnosticRow> {
    let row = |d: &smd_api::ScanDiagnostic| DiagnosticRow {
        code: d.code.clone(),
        level: d.level,
        location: d.location.clone(),
        detail: d.detail.clone(),
    };
    found.diagnostics.iter().map(row).collect()
}

/// Print `rows` to `err` as a table; nothing when there are none.
fn warn_rows(c: &Common, env: &Env, err: &mut dyn Write, rows: &[DiagnosticRow]) {
    if !rows.is_empty() {
        let opts = options(c, env, Caps::default());
        let _ = write!(err, "{}", render_diagnostics(DiagnosticsFormat::Table, rows, &opts));
    }
}

/// A `--game-dir` / `--user-dir` that is not a directory is a command that could not run
/// (exit 1), not a location that was merely not found.
fn check_given_dirs(c: &Common, env: &Env, err: &mut dyn Write) -> Result<(), i32> {
    let given = [("--game-dir", "game", &c.game_dir), ("--user-dir", "user", &c.user_dir)];
    for (flag, what, dir) in given {
        let Some(p) = dir.as_ref().filter(|p| !p.is_dir()) else { continue };
        let row = DiagnosticRow {
            code: "smd.dir_missing".to_owned(),
            level: DiagSeverity::Error,
            location: flag.to_owned(),
            detail: format!(
                "cannot read the {what} directory: given directory does not exist: {}",
                p.display()
            ),
        };
        warn_rows(c, env, err, &[row]);
        return Err(FAILED);
    }
    Ok(())
}

fn scan(a: &ScanArgs, env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (game, user, workshops, gap) = match resolve(&a.common, env, err) {
        Ok(roots) => roots,
        Err(code) => return code,
    };
    let mut req = ScanRequest::new(game, user);
    req.workshop_dirs = workshops;
    req.locations_incomplete = gap;
    req.game.clone_from(&a.common.game);
    if a.progress || std::io::stderr().is_terminal() {
        // Straight to the process stderr: the callback must be `Send + Sync`, `err` is not.
        req.progress = Some(ProgressFn(Arc::new(|p| {
            let label = if p.name.is_empty() { &p.key } else { &p.name };
            eprintln!("[{}/{}] {label}", p.index, p.total);
        })));
    }
    let outcome = match smd_api::execute(Command::Scan(req)) {
        Ok(Outcome::Scan(o)) => o,
        Ok(Outcome::Locate(_)) => return FAILED,
        Err(e) => return fail(&a.common, env, err, "smd.scan_failed", &e),
    };
    let had_error = outcome.diagnostics.iter().any(|d| d.level == DiagSeverity::Error);
    let rendered = render(a, env, outcome);
    emit(out, err, &rendered, had_error)
}

fn render(a: &ScanArgs, env: &Env, outcome: smd_api::ScanOutcome) -> String {
    let defaults = Caps::default();
    let caps = Caps {
        max_participants: a.max_participants.unwrap_or(defaults.max_participants),
        max_findings: a.max_findings.unwrap_or(defaults.max_findings),
    };
    let opts = options(&a.common, env, caps);
    let model = report_model(outcome);
    match a.common.format {
        Format::Table => Table.render(&model, &opts),
        Format::Json => Json.render(&model, &opts),
        Format::Markdown => Markdown.render(&model, &opts),
    }
}
