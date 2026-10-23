//! The custom scheme's request path: Tauri's request in, the router's out.

use axum::body::Body;

/// Answer one scheme request from the router.
///
/// Tauri's request types are the `http` crate's, so this only moves the body
/// into axum's; the request path itself is `nigel_core::server::scheme::answer`,
/// which the Swift shell's `nigel-ffi` shares.
pub async fn answer(
    router: axum::Router,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let (parts, body) = request.into_parts();
    let request = axum::http::Request::from_parts(parts, Body::from(body));
    nigel_core::server::scheme::answer(router, request).await
}

#[cfg(test)]
mod tests {
    use axum::routing::get;

    #[tokio::test]
    async fn it_answers_from_the_router_with_status_headers_and_body() {
        let router = axum::Router::new().route(
            "/hello",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    "hi there",
                )
            }),
        );

        let request = tauri::http::Request::builder()
            .uri("nigel://localhost/hello")
            .method("GET")
            .body(Vec::new())
            .unwrap();

        let response = super::answer(router, request).await;

        assert_eq!(response.status(), 200);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
        assert_eq!(response.body(), b"hi there");
    }

    #[tokio::test]
    async fn it_carries_the_request_method_body_and_headers_through() {
        let router = axum::Router::new().route(
            "/echo",
            axum::routing::post(|headers: axum::http::HeaderMap, body: String| async move {
                format!(
                    "{} {}",
                    headers.get("x-probe").unwrap().to_str().unwrap(),
                    body
                )
            }),
        );

        let request = tauri::http::Request::builder()
            .uri("nigel://localhost/echo")
            .method("POST")
            .header("x-probe", "seen")
            .body(b"payload".to_vec())
            .unwrap();

        let response = super::answer(router, request).await;

        assert_eq!(response.body(), b"seen payload");
    }
}
