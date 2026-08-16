//! What may reach into the terminal UI, and what may not.
//!
//! `src/server/` is the half a desktop client links without a terminal (see
//! backlog/decisions/decision-1). Every `crate::cli::` reference here is a
//! build error waiting for the workspace split in TASK-33.1, so it is one
//! here first, where the fix is cheap.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories that must not reach into the CLI/TUI layer.
const CORE_DIRS: [&str; 1] = ["src/server"];

/// Test support that drives the CLI's own formatters on purpose: the figure
/// parity fixtures compare what a browser renders against what `nigel invoice
/// list` prints, which means naming both. Neither ships in a release binary.
/// Both move to the CLI crate when the workspace splits.
const TEST_SUPPORT: [&str; 2] = ["testutil.rs", "fixture_capture.rs"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !TEST_SUPPORT.contains(&name.as_str()) {
                out.push(path);
            }
        }
    }
}

fn cli_references() -> Vec<String> {
    let mut files = Vec::new();
    for dir in CORE_DIRS {
        rust_files(Path::new(dir), &mut files);
    }
    files.sort();

    let mut hits = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("read source file");
        for (number, line) in text.lines().enumerate() {
            if line.contains("crate::cli::") {
                hits.push(format!("{}:{}: {}", file.display(), number + 1, line.trim()));
            }
        }
    }
    hits
}

// Red until Task 11 lands. Each task in the plan removes its own module's
// references; running this test is how a task proves it finished.
#[ignore = "red until the boundary move completes (TASK-33.1)"]
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
