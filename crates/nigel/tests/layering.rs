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
/// crate; `nigel::` and `extern crate nigel` are what one would look like
/// now. All are checked, because a file moved back from history carries the
/// old spelling, and a bare `extern crate nigel;` names the CLI without ever
/// spelling `nigel::`.
fn names_the_cli(line: &str) -> bool {
    line.contains("crate::cli::")
        || line.contains("crate::cli;")
        || line.contains("crate::{cli")
        || line.contains("crate::{ cli")
        || line.contains("nigel::")
        || line.contains("extern crate nigel")
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
///
/// This is not redundant with the compiler. `cargo build -p nigel-core`
/// does not need dev- or build-dependencies to succeed, and Cargo allows a
/// dev-dependency cycle between workspace members outright — that is how two
/// crates' test suites are permitted to depend on each other. A `nigel`
/// dev-dependency here would let `nigel-core`'s `#[cfg(test)]` code reach
/// into the CLI crate while `cargo build -p nigel-core` and even
/// `cargo test -p nigel-core` both stayed green. So every dependency table
/// Cargo recognizes is checked, not just `[dependencies]`, and both the
/// inline (`nigel = "1"`) and section (`[dependencies.nigel]`) spellings are
/// caught by parsing the manifest as TOML rather than scanning lines by hand.
#[test]
fn the_core_manifest_does_not_depend_on_the_binary_crate() {
    let manifest =
        fs::read_to_string("../nigel-core/Cargo.toml").expect("read nigel-core manifest");
    let manifest: toml::Table = manifest.parse().expect("parse nigel-core manifest as TOML");

    let mut hits = Vec::new();
    check_dependency_tables(&manifest, "", &mut hits);

    assert!(
        hits.is_empty(),
        "nigel-core depends on the binary crate in: {}",
        hits.join(", ")
    );
}

/// Checks `dependencies`, `dev-dependencies` and `build-dependencies` directly
/// under `table` for an edge to `nigel` — either as the key, or renamed under a
/// different key with `package = "nigel"` — then recurses into `table.target.*`,
/// where Cargo allows the same three tables to reappear per build target.
fn check_dependency_tables(table: &toml::Table, path: &str, hits: &mut Vec<String>) {
    for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(toml::Value::Table(deps)) = table.get(kind) {
            for (name, spec) in deps {
                let renamed = spec
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|package| package == "nigel");
                if name == "nigel" || renamed {
                    hits.push(format!("[{path}{kind}] {name}"));
                }
            }
        }
    }
    if let Some(toml::Value::Table(targets)) = table.get("target") {
        for (spec, target_table) in targets {
            if let toml::Value::Table(target_table) = target_table {
                check_dependency_tables(target_table, &format!("target.{spec}."), hits);
            }
        }
    }
}

/// No workspace member may switch on `nigel-core`'s `desktop` feature.
///
/// That feature carries `build_desktop_router`, which has no session guard.
/// Cargo unifies features across workspace members, so a single member asking
/// for it compiles that router into the `nigel-core` the `nigel` binary links —
/// and the binary stays clean only because nothing references it and the linker
/// drops it. The gate is meant to be structural, not a bet on dead-code
/// elimination, so the desktop crate lives outside this workspace entirely and
/// this test is what notices if it is ever pulled back in.
#[test]
fn no_workspace_member_enables_the_desktop_feature() {
    let root: toml::Table = fs::read_to_string("../../Cargo.toml")
        .expect("read workspace manifest")
        .parse()
        .expect("parse workspace manifest");

    let members = root["workspace"]["members"]
        .as_array()
        .expect("workspace members");

    let mut hits = Vec::new();
    for member in members {
        let dir = member.as_str().expect("member path");
        let manifest: toml::Table = fs::read_to_string(format!("../../{dir}/Cargo.toml"))
            .expect("read member manifest")
            .parse()
            .expect("parse member manifest");

        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let Some(toml::Value::Table(deps)) = manifest.get(kind) else {
                continue;
            };
            for (name, spec) in deps {
                let is_core = name == "nigel-core"
                    || spec.get("package").and_then(toml::Value::as_str) == Some("nigel-core");
                let asks_for_desktop = spec
                    .get("features")
                    .and_then(toml::Value::as_array)
                    .is_some_and(|f| f.iter().any(|v| v.as_str() == Some("desktop")));
                if is_core && asks_for_desktop {
                    hits.push(format!("{dir} [{kind}] {name}"));
                }
            }
        }
    }

    assert!(
        hits.is_empty(),
        "these workspace members enable nigel-core's desktop feature: {}",
        hits.join(", ")
    );
}
