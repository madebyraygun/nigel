//! Who may call what until the role table replaces this module's guard.
//!
//! Every `/api` route needs an admin except the few a signed-in user needs
//! for their own session and account, so a user who is not an admin can do
//! nothing with the books while roles are unfinished. The local owner — a
//! local-mode token session or the desktop transport — passes everything.

use axum::extract::Request;
use axum::http::Method;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::auth::LOGIN_PATH;
use super::error::ApiError;
use super::principal::Principal;
use crate::identity::Role;

/// What any signed-in role may call, as seen inside the `/api` nest.
const ANY_ROLE: [(Method, &str); 7] = [
    (Method::GET, "/ping"),
    (Method::GET, "/auth/me"),
    (Method::POST, "/auth/logout"),
    (Method::POST, "/account/password"),
    (Method::POST, "/account/totp/setup"),
    (Method::POST, "/account/totp/confirm"),
    (Method::POST, "/account/totp/remove"),
];

/// Layered inside the session layer (or the desktop router's principal), so
/// a request with no principal is refused rather than let through.
pub async fn interim_admin_guard(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == LOGIN_PATH {
        return next.run(req).await;
    }
    let allowed = match req.extensions().get::<Principal>() {
        None => return ApiError::unauthorized().into_response(),
        Some(principal) => {
            principal.satisfies(Role::Admin)
                || ANY_ROLE
                    .iter()
                    .any(|(m, p)| m == req.method() && *p == path)
        }
    };
    if allowed {
        next.run(req).await
    } else {
        ApiError::forbidden_role(Role::Admin).into_response()
    }
}

/// The desktop router's principal: it has no session layer, and its
/// requests run as the local owner whatever the books hold.
pub async fn as_local_owner(mut req: Request, next: Next) -> Response {
    req.extensions_mut().insert(Principal::LocalOwner);
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::{middleware, Router};
    use tower::ServiceExt;

    fn guarded() -> Router {
        Router::new()
            .route("/accounts", get(|| async { "ok" }))
            .route(LOGIN_PATH, post(|| async { "ok" }))
            .layer(middleware::from_fn(interim_admin_guard))
    }

    async fn status(method: Method, uri: &str) -> StatusCode {
        guarded()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
            .status()
    }

    #[tokio::test]
    async fn a_request_with_no_principal_is_refused_except_the_login() {
        assert_eq!(
            status(Method::GET, "/accounts").await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(status(Method::POST, LOGIN_PATH).await, StatusCode::OK);
    }
}
