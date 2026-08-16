//! What may reach into the terminal UI, and what may not.
//!
//! Everything under `src/` is the half a desktop client links without a
//! terminal, except the four entries named below. `src/cli/` is clap and
//! ratatui, so anything on the core side that names `crate::cli::` cannot be
//! linked without them, and this test says so. The reasoning is recorded in
//! `backlog/decisions/decision-1`.
//!
//! The scope is stated as what is **excluded** rather than what is covered: a
//! module added to the core side is then guarded from the moment it exists.
//! A hand-listed set of covered paths guards only what somebody remembered,
//! which is how `migrations.rs` reached `crate::cli::today()` on the server's
//! own unlock path while this test read zero.

use std::fs;
use std::path::{Path, PathBuf};

/// The terminal UI, and the only part of `src/` allowed to name `crate::cli`.
///
/// `tui`, `browser` and `effects` are ratatui screens and the animation they
/// share; `main.rs` is the binary's own entry point. Everything else under
/// `src/` is core and is guarded.
const CLI_PATHS: [&str; 5] = [
    "src/cli",
    "src/tui.rs",
    "src/browser.rs",
    "src/effects.rs",
    "src/main.rs",
];

/// Test support the server's own tests build on: seeded databases, a router
/// with a valid session, and JSON request helpers. It stays out of a release
/// binary, but it is not itself CLI-shaped, so it is excluded by name rather
/// than folded into [`CLI_PATHS`].
///
/// Matched by its full path from the repo root, not by bare filename: a
/// bare-filename match would also exclude any future `testutil.rs` dropped
/// into an unrelated core directory, leaving it silently unguarded.
const TEST_SUPPORT: [&str; 1] = ["src/server/testutil.rs"];

fn rel(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Collect every `.rs` file under `path` that is neither CLI nor test support.
fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
    let name = rel(path);
    if CLI_PATHS.contains(&name.as_str()) || TEST_SUPPORT.contains(&name.as_str()) {
        return;
    }

    if path.is_dir() {
        for entry in fs::read_dir(path).expect("read_dir") {
            rust_files(&entry.expect("dir entry").path(), out);
        }
    } else if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
    {
        out.push(path.to_path_buf());
    }
}

/// Every way a line can name the CLI module.
///
/// The bare path is the common one. The braced forms are what a nested `use`
/// group produces — `use crate::{cli::invoice::x, fmt::money};` contains no
/// `crate::cli::` at all, so grepping only for that reads clean while the
/// import is right there.
fn names_the_cli(line: &str) -> bool {
    line.contains("crate::cli::")
        || line.contains("crate::cli;")
        || line.contains("crate::{cli")
        || line.contains("crate::{ cli")
}

fn cli_references() -> Vec<String> {
    let mut files = Vec::new();
    let src = Path::new("src");
    assert!(
        src.is_dir(),
        "src/ is not a directory — this test must run from the crate root"
    );
    rust_files(src, &mut files);
    assert!(
        !files.is_empty(),
        "walked src/ and found no core files to check — the exclusion list has \
         swallowed the whole crate"
    );
    files.sort();

    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if names_the_cli(line) {
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
fn the_core_does_not_reach_into_the_cli_layer() {
    let hits = cli_references();
    assert!(
        hits.is_empty(),
        "the core side still reaches into the CLI layer in {} place(s):\n{}",
        hits.len(),
        hits.join("\n")
    );
}

/// The exclusions are paths, and a path that no longer exists excludes nothing
/// while looking like it does. A rename would otherwise quietly pull the CLI
/// into the guarded set, and the whole suite would still pass.
#[test]
fn every_excluded_path_exists() {
    for path in CLI_PATHS.iter().chain(TEST_SUPPORT.iter()) {
        assert!(
            Path::new(path).exists(),
            "excluded path {path} does not exist — rename it here, or it stops \
             excluding anything and the guard's scope changes silently"
        );
    }
}
