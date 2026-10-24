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

/// The library's unit tests run against a config directory of their own.
///
/// `nigel_core`'s `cfg(test)` fallback covers only its own tests; this binary
/// links it with the `testutil` feature, where an unredirected test would read
/// and rewrite the developer's real `~/.config/nigel/settings.json`.
#[cfg(test)]
mod test_config_sandbox {
    use std::path::PathBuf;

    fn sandbox() -> PathBuf {
        std::env::temp_dir().join(format!("nigel-lib-test-config-{}", std::process::id()))
    }

    #[ctor::ctor(unsafe)]
    fn redirect_the_config_directory() {
        nigel_core::settings::set_config_dir_for_tests(Some(sandbox()));
    }

    #[test]
    fn an_unguarded_test_sees_the_sandbox_not_the_real_config() {
        assert_eq!(nigel_core::settings::config_dir(), sandbox());
        let real =
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("nigel"));
        assert_ne!(Some(nigel_core::settings::config_dir()), real);
    }
}
