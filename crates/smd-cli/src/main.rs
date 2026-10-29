//! The `smd` binary: the process environment, and nothing else.

use std::io::IsTerminal;

use smd_cli::Env;
use smd_report::RedactionContext;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let env = Env {
        redaction: RedactionContext::from_env(),
        stdout_is_tty: std::io::stdout().is_terminal(),
    };
    let code = smd_cli::run(&argv, &env, &mut std::io::stdout().lock(), &mut std::io::stderr());
    std::process::exit(code);
}
