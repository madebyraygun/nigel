//! What may reach into the terminal UI, and what may not.
//!
//! The paths below are the half of this crate a desktop client links without a
//! terminal. `src/cli/` is clap and ratatui, so anything under those paths that
//! names `crate::cli::` cannot be linked without them, and this test says so.

use std::fs;
use std::path::{Path, PathBuf};

/// The files and directories that must not reach into the CLI/TUI layer.
const CORE_PATHS: [&str; 11] = [
    "src/server",
    "src/reports",
    "src/invoicing",
    "src/accounts.rs",
    "src/categories.rs",
    "src/rules.rs",
    "src/imports.rs",
    "src/backup.rs",
    "src/password.rs",
    "src/updater.rs",
    "src/clock.rs",
];

/// Test support that drives the CLI's own formatters on purpose: the figure
/// parity fixtures compare what a browser renders against what `nigel invoice
/// list` prints, which means naming both. Neither ships in a release binary.
///
/// Matched by their full path from the repo root, not by bare filename: a
/// bare-filename match would also exclude any future `testutil.rs` dropped
/// into an unrelated core directory, leaving it silently unguarded.
const TEST_SUPPORT: [&str; 2] = ["src/server/testutil.rs", "src/server/fixture_capture.rs"];

/// Collect every `.rs` file under `path`, or `path` itself if it already
/// names one, skipping `TEST_SUPPORT`.
fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read_dir") {
            rust_files(&entry.expect("dir entry").path(), out);
        }
    } else if path.extension().is_some_and(|ext| ext == "rs") {
        let rel = path.to_string_lossy().replace('\\', "/");
        if !TEST_SUPPORT.contains(&rel.as_str()) {
            out.push(path.to_path_buf());
        }
    }
}

fn cli_references() -> Vec<String> {
    let mut files = Vec::new();
    for path in CORE_PATHS {
        let path = Path::new(path);
        // A directory that has been renamed away is neither a directory nor a
        // `.rs` file, so `rust_files` would walk nothing and this test would
        // pass having checked that entry not at all.
        assert!(
            path.exists(),
            "CORE_PATHS names {}, which does not exist — rename it here or the \
             guard silently stops covering it",
            path.display()
        );
        rust_files(path, &mut files);
    }
    files.sort();

    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if line.contains("crate::cli::") {
                hits.push(format!(
                    "{}:{}: {}",
                    file.display(),
                    number + 1,
                    line.trim()
                ));
            }
        }
    }
    hits
}

#[test]
fn the_server_does_not_reach_into_the_cli_layer() {
    let hits = cli_references();
    assert!(
        hits.is_empty(),
        "the server still reaches into the CLI layer in {} place(s):\n{}",
        hits.len(),
        hits.join("\n")
    );
}
