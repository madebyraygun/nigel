//! Nigel's terminal UI — the half of the program that needs a terminal.
//!
//! Command dispatch, the clap definitions, the ratatui screens and the
//! animation they share. Everything it operates on lives in `nigel_core`; the
//! `nigel` binary (`src/main.rs`) is a thin shell over this crate — clap
//! parsing, the dispatch pre-flight, and the terminal-restoring panic hook.

pub mod browser;
pub mod cli;
pub mod effects;
pub mod tui;
