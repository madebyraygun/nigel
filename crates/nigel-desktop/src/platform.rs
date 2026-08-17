//! Tells the page which OS it is running under.
//!
//! `navigator.userAgent` is not a reliable platform signal inside a webview,
//! so the page asks the native side instead.

/// `std::env::consts::OS` — `"linux"`, `"macos"`, `"windows"`, and so on.
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_answers_the_rust_compile_target_os() {
        assert_eq!(platform(), std::env::consts::OS);
    }
}
