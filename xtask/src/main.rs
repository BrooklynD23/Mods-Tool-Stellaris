//! `cargo xtask` — the workspace task runner.
//!
//! `cargo xtask lint` is the mechanical half of ADR-0002. A constraint that is not
//! enforced by CI is a preference, and a size limit added later is a limit that never
//! gets enforced — so these gates are live from commit one, on a tree that is mostly
//! stubs.

mod codelines;
mod lint;

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("lint") => lint::run(),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}`");
            usage();
            ExitCode::FAILURE
        }
        None => {
            usage();
            ExitCode::FAILURE
        }
    }
}

fn usage() {
    eprintln!("usage: cargo xtask <task>");
    eprintln!();
    eprintln!("tasks:");
    eprintln!("  lint    structural gates: size, layers, game names, doc links, profile");
}
