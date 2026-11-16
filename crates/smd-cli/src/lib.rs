//! `smd-cli` — The command-line adapter over the engine.
//!
//! **Owns:** Argument parsing and terminal output, over `smd-api` and `smd-report`.
//!
//! **Must never:** Contain domain logic. The engine is a library first; the CLI is a thin adapter.
//!
//! # What exists so far
//!
//! `smd scan`. [`run`] parses hand-rolled arguments ([`args`]), runs the scan through
//! `smd-api`, maps the text-only outcome onto `smd-report`'s model ([`model`]), and prints
//! the report. The binary (`src/main.rs`) only supplies the process environment.

#![forbid(unsafe_code)]

pub mod args;
pub mod model;
mod run;

pub use run::{Env, run};
