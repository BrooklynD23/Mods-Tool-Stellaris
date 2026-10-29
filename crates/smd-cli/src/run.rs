//! Running a command line: parse, scan, render, exit code.

use std::io::Write;

use smd_api::{Command, Outcome, ScanRequest};
use smd_core::DiagSeverity;
use smd_report::{
    Caps, DiagnosticRow, DiagnosticsFormat, Json, Markdown, RedactionContext, RenderOptions,
    Renderer, Table, render_diagnostics,
};

use crate::args::{self, Format, Parsed, ScanArgs, USAGE};
use crate::model::report_model;

/// The process environment the CLI reads, passed in so tests need not touch the real one.
#[derive(Clone, Debug)]
pub struct Env {
    /// What redaction removes.
    pub redaction: RedactionContext,
    /// Whether stdout is a terminal; colour is on only then.
    pub stdout_is_tty: bool,
}

/// Exit code for a completed scan.
const OK: i32 = 0;
/// Exit code for a scan that finished but emitted an Error-class diagnostic (a file or
/// directory could not be read). Never used because conflicts were found.
const DIAGNOSTIC_ERROR: i32 = 3;
/// Exit code for a scan that could not run.
const FAILED: i32 = 1;
/// Exit code for a bad command line.
const USAGE_ERROR: i32 = 2;

/// Run `smd` with `argv` (the arguments after the program name). Returns the exit code.
pub fn run(argv: &[String], env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    match args::parse(argv) {
        Ok(Parsed::Help) => {
            let _ = writeln!(out, "{USAGE}");
            OK
        }
        Ok(Parsed::Scan(a)) => scan(&a, env, out, err),
        Err(msg) => {
            let _ = writeln!(err, "smd: {msg}\n\n{USAGE}");
            USAGE_ERROR
        }
    }
}

fn scan(a: &ScanArgs, env: &Env, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let mut req = ScanRequest::new(a.game_dir.clone(), a.user_dir.clone());
    req.workshop_dirs.clone_from(&a.workshop_dirs);
    let outcome = match smd_api::execute(Command::Scan(req)) {
        Ok(Outcome::Scan(o)) => o,
        Err(e) => {
            let row = DiagnosticRow {
                code: "smd.scan_failed".to_owned(),
                level: DiagSeverity::Error,
                location: String::new(),
                detail: e.to_string(),
            };
            let opts = options(a, env, Caps::default());
            let _ = write!(err, "{}", render_diagnostics(DiagnosticsFormat::Table, &[row], &opts));
            return FAILED;
        }
    };
    let had_error = outcome.diagnostics.iter().any(|d| d.level == DiagSeverity::Error);
    let rendered = render(a, env, outcome);
    match out.write_all(rendered.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) if had_error => DIAGNOSTIC_ERROR,
        Ok(()) => OK,
        Err(e) => {
            let _ = writeln!(err, "smd: cannot write the report: {}", e.kind());
            FAILED
        }
    }
}

fn options(a: &ScanArgs, env: &Env, caps: Caps) -> RenderOptions {
    let mut opts =
        RenderOptions::new(env.redaction.clone()).with_color(env.stdout_is_tty).with_caps(caps);
    if !a.redact {
        opts = opts.without_redaction();
    }
    opts
}

fn render(a: &ScanArgs, env: &Env, outcome: smd_api::ScanOutcome) -> String {
    let defaults = Caps::default();
    let caps = Caps {
        max_participants: a.max_participants.unwrap_or(defaults.max_participants),
        max_findings: a.max_findings.unwrap_or(defaults.max_findings),
    };
    let opts = options(a, env, caps);
    let model = report_model(outcome);
    match a.format {
        Format::Table => Table.render(&model, &opts),
        Format::Json => Json.render(&model, &opts),
        Format::Markdown => Markdown.render(&model, &opts),
    }
}
