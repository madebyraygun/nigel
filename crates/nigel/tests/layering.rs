//! What may reach into the terminal UI, and what may not.
//!
//! `nigel-core` is the half a desktop client links without a terminal;
//! `nigel` is clap and ratatui. The crate boundary is the real enforcement —
//! `nigel-core` does not depend on `nigel`, so a core module naming the CLI
//! does not compile. This test guards the boundary in source terms as well, so
//! a stray reference is reported as what it is rather than as a resolution
//! error, and so the rule survives anyone tempted to add the back-edge. The
//! reasoning is recorded in `backlog/decisions/decision-1`.
//!
//! The scope is every `.rs` file in the core crate: a module added there is
//! guarded from the moment it exists, with nothing to remember to list.

use std::fs;
use std::path::{Path, PathBuf};

/// The core crate's sources, relative to this crate's manifest directory.
const CORE_SRC: &str = "../nigel-core/src";

fn rust_files(path: &Path, out: &mut Vec<PathBuf>) {
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

/// Every way a line can name the terminal UI.
///
/// `crate::cli` is what a reference looked like while the two halves shared a
/// crate; `nigel::` is what one would look like now. Both are checked, because
/// a file moved back from history carries the old spelling.
fn names_the_cli(line: &str) -> bool {
    line.contains("crate::cli::")
        || line.contains("crate::cli;")
        || line.contains("crate::{cli")
        || line.contains("crate::{ cli")
        || line.contains("nigel::")
}

fn cli_references() -> Vec<String> {
    let mut files = Vec::new();
    let src = Path::new(CORE_SRC);
    assert!(
        src.is_dir(),
        "{CORE_SRC} is not a directory — this test must run from the crate root"
    );
    rust_files(src, &mut files);
    assert!(
        !files.is_empty(),
        "walked {CORE_SRC} and found no core files to check"
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

/// The core crate must not depend on the binary crate, in either direction of
/// reading: a `nigel` entry in `nigel-core`'s manifest would make the source
/// check above pass while the boundary was already gone.
#[test]
fn the_core_manifest_does_not_depend_on_the_binary_crate() {
    let manifest =
        fs::read_to_string("../nigel-core/Cargo.toml").expect("read nigel-core manifest");
    for line in manifest.lines() {
        let name = line.split(['=', ' ']).next().unwrap_or("").trim();
        assert_ne!(
            name,
            "nigel",
            "nigel-core depends on the binary crate: {}",
            line.trim()
        );
    }
}
