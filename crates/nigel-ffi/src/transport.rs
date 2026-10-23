//! The scheme request and response as plain records the foreign side can
//! build and read, and their conversion to and from `http`.
//!
//! Nothing here knows the request will be answered in process: the records
//! are an HTTP exchange with nothing local-specific in them, so a transport
//! that answers the same `nigel://` request from a remote `nigel serve` speaks
//! the same shapes.

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, Method, Request, Response, Uri};

/// One header. A list of these rather than a map, so a repeated header
/// (`Set-Cookie`, `Vary`) survives the crossing in order.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SchemeHeader {
    pub name: String,
    pub value: String,
}

/// A request the webview made of the scheme.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SchemeRequest {
    pub method: String,
    /// The whole URL the webview requested, authority included
    /// (`nigel://localhost/api/status`): the authority is what becomes the
    /// `Host` the router's guard reads.
    pub url: String,
    pub headers: Vec<SchemeHeader>,
    pub body: Vec<u8>,
}

/// The router's answer: status, headers and the whole body.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SchemeResponse {
    pub status: u16,
    pub headers: Vec<SchemeHeader>,
    pub body: Vec<u8>,
}

impl SchemeResponse {
    /// A plain-text answer for a request that never reached the router.
    pub(crate) fn plain(status: u16, message: String) -> Self {
        Self {
            status,
            headers: vec![SchemeHeader {
                name: "content-type".to_string(),
                value: "text/plain; charset=utf-8".to_string(),
            }],
            body: message.into_bytes(),
        }
    }
}

impl TryFrom<SchemeRequest> for Request<Body> {
    type Error = String;

    fn try_from(request: SchemeRequest) -> Result<Self, Self::Error> {
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|_| format!("not an HTTP method: {}", request.method))?;
        let uri: Uri = request
            .url
            .parse()
            .map_err(|e| format!("not a URL: {}: {e}", request.url))?;
        let mut builder = Request::builder().method(method).uri(uri);
        for header in request.headers {
            let name = HeaderName::from_bytes(header.name.as_bytes())
                .map_err(|_| format!("not a header name: {}", header.name))?;
            let value = HeaderValue::from_str(&header.value)
                .map_err(|_| format!("not a header value for {}", header.name))?;
            builder = builder.header(name, value);
        }
        builder
            .body(Body::from(request.body))
            .map_err(|e| e.to_string())
    }
}

impl From<Response<Vec<u8>>> for SchemeResponse {
    fn from(response: Response<Vec<u8>>) -> Self {
        let (parts, body) = response.into_parts();
        let headers = parts
            .headers
            .iter()
            .map(|(name, value)| SchemeHeader {
                name: name.as_str().to_string(),
                value: String::from_utf8_lossy(value.as_bytes()).into_owned(),
            })
            .collect();
        Self {
            status: parts.status.as_u16(),
            headers,
            body,
        }
    }
}

/// Answer one scheme request from the router.
///
/// A request the `http` crate cannot represent never reaches the router and
/// is answered 400, so a malformed request from the foreign side is a response
/// it can show rather than an error it has to map.
pub async fn answer(router: axum::Router, request: SchemeRequest) -> SchemeResponse {
    match Request::<Body>::try_from(request) {
        Ok(request) => nigel_core::server::scheme::answer(router, request)
            .await
            .into(),
        Err(message) => SchemeResponse::plain(400, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(method: &str, url: &str) -> SchemeRequest {
        SchemeRequest {
            method: method.to_string(),
            url: url.to_string(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn a_request_converts_with_its_method_headers_and_body() {
        let mut scheme = request("POST", "nigel://localhost/api/echo");
        scheme.headers.push(SchemeHeader {
            name: "x-probe".to_string(),
            value: "seen".to_string(),
        });
        scheme.body = b"payload".to_vec();

        let converted = Request::<Body>::try_from(scheme).expect("converts");

        assert_eq!(converted.method(), Method::POST);
        assert_eq!(converted.uri().authority().unwrap().as_str(), "localhost");
        assert_eq!(converted.headers()["x-probe"], "seen");
    }

    #[test]
    fn a_repeated_response_header_keeps_every_value() {
        let response = Response::builder()
            .status(201)
            .header("vary", "origin")
            .header("vary", "accept")
            .body(b"made".to_vec())
            .unwrap();

        let converted = SchemeResponse::from(response);

        assert_eq!(converted.status, 201);
        let vary: Vec<_> = converted
            .headers
            .iter()
            .filter(|h| h.name == "vary")
            .map(|h| h.value.as_str())
            .collect();
        assert_eq!(vary, ["origin", "accept"]);
        assert_eq!(converted.body, b"made");
    }

    #[test]
    fn a_malformed_request_is_answered_400_without_reaching_the_router() {
        let router = axum::Router::new();

        let bad_method = pollster::block_on(answer(
            router.clone(),
            request("NOT A METHOD", "nigel://localhost/"),
        ));
        let bad_url = pollster::block_on(answer(router, request("GET", "nigel://local host/")));

        assert_eq!(bad_method.status, 400);
        assert_eq!(bad_url.status, 400);
        assert!(String::from_utf8_lossy(&bad_url.body).contains("not a URL"));
    }
}
