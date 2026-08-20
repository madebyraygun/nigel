//! The shell asks for a floor under the window size. Neither the setup gate's
//! four steps nor the shell's sidebar-plus-table survives a 400px window, and
//! a webview will happily be dragged to one.
//!
//! Read off the source rather than a running window: building one needs a
//! display server, which CI has not got.

use std::fs;

#[test]
fn the_window_declares_a_minimum_size() {
    let main = fs::read_to_string("src/main.rs").expect("read main.rs");

    assert!(
        main.contains(".min_inner_size(900.0, 700.0)"),
        "src/main.rs does not set a minimum inner size"
    );
    let min_at = main.find(".min_inner_size").expect("min_inner_size");
    let build_at = main.find(".build()?").expect("build()");
    assert!(
        min_at < build_at,
        "min_inner_size is not applied to the window builder"
    );
}
