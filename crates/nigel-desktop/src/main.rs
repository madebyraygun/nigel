//! The desktop shell: nigel's SPA and JSON API over one custom URI scheme.

use tauri::{WebviewUrl, WebviewWindowBuilder};

/// The scheme the SPA and the API are both served from.
const SCHEME: &str = "nigel";

/// The origin form Tauri gives a custom scheme, which differs by platform.
fn scheme_url() -> String {
    if cfg!(windows) {
        format!("http://{SCHEME}.localhost/")
    } else {
        format!("{SCHEME}://localhost/")
    }
}

fn main() {
    tauri::Builder::default()
        .register_asynchronous_uri_scheme_protocol(SCHEME, move |_ctx, _request, responder| {
            responder.respond(
                tauri::http::Response::builder()
                    .header(tauri::http::header::CONTENT_TYPE, "text/html")
                    .body(b"<title>nigel</title><p>scheme reached".to_vec())
                    .expect("build response"),
            );
        })
        .setup(|app| {
            WebviewWindowBuilder::new(
                app,
                "main",
                WebviewUrl::CustomProtocol(scheme_url().parse().expect("scheme url")),
            )
            .title("Nigel")
            .inner_size(1200.0, 820.0)
            .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("run nigel desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_url_matches_the_platform_origin_form() {
        // Tauri serves a custom scheme from a different origin per platform.
        // Getting this wrong means the host guard refuses every request.
        let url = scheme_url();
        if cfg!(windows) {
            assert_eq!(url, "http://nigel.localhost/");
        } else {
            assert_eq!(url, "nigel://localhost/");
        }
        assert!(url.starts_with(SCHEME) || url.contains(SCHEME));
    }
}
