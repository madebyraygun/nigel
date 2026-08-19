//! The custom scheme is a transport, not something other applications may open.

use std::fs;

#[test]
fn the_scheme_is_not_registered_as_a_deep_link() {
    let config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string("tauri.conf.json").expect("read config"))
            .expect("parse config");

    // Tauri registers deep links through the plugin's config block. Any
    // presence of it means another program could hand us a nigel:// URL.
    assert!(
        config["plugins"]["deep-link"].is_null(),
        "tauri.conf.json registers a deep link: {}",
        config["plugins"]["deep-link"]
    );

    let manifest = fs::read_to_string("Cargo.toml").expect("read manifest");
    assert!(
        !manifest.contains("tauri-plugin-deep-link"),
        "the deep-link plugin is a dependency"
    );
}
