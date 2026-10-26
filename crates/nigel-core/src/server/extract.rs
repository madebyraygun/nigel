//! Extractors that answer inside the error envelope.
//!
//! axum's own `Json` and `Path` rejections are `text/plain`, which would make
//! them the only responses on the API a client cannot parse the same way as
//! every other failure. These wrappers are the same extractors with the
//! rejection translated.
//!
//! A body that carries a password or a code never goes through `ApiJson`: its
//! rejection text would quote the offending value. `POST /api/unlock` and
//! `POST /api/setup` read theirs by hand; the sign-in routes use
//! [`SecretJson`].

use std::net::SocketAddr;

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{ConnectInfo, FromRequest, FromRequestParts, Path, Request};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Json;
use serde::de::DeserializeOwned;

use super::error::ApiError;

/// `Json<T>`, with a 400 in the error envelope instead of a plain-text body.
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(bad_json(&rejection)),
        }
    }
}

fn bad_json(rejection: &JsonRejection) -> ApiError {
    ApiError::bad_request(match rejection {
        JsonRejection::MissingJsonContentType(_) => {
            "Expected a JSON body with Content-Type: application/json.".to_string()
        }
        other => other.body_text(),
    })
}

/// `Path<T>`, with a 400 in the error envelope instead of a plain-text body.
pub struct ApiPath<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiPath<T>
where
    S: Send + Sync,
    T: DeserializeOwned + Send,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Path::<T>::from_request_parts(parts, state).await {
            Ok(Path(value)) => Ok(Self(value)),
            Err(rejection) => Err(bad_path(&rejection)),
        }
    }
}

fn bad_path(rejection: &PathRejection) -> ApiError {
    ApiError::bad_request(match rejection {
        PathRejection::FailedToDeserializePathParams(_) => {
            "Expected a numeric id in the path.".to_string()
        }
        other => other.body_text(),
    })
}

/// `Json<T>` for bodies that carry a password or a code. Every rejection is
/// a fixed sentence that quotes nothing from the body, and a body over the
/// route's limit is a `413`.
pub struct SecretJson<T>(pub T);

impl<S, T> FromRequest<S> for SecretJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => Err(
                ApiError::payload_too_large("That request body is too large."),
            ),
            Err(JsonRejection::MissingJsonContentType(_)) => Err(ApiError::bad_request(
                "Expected a JSON body with Content-Type: application/json.",
            )),
            Err(_) => Err(ApiError::bad_request(
                "That request body is not the JSON this endpoint expects.",
            )),
        }
    }
}

/// The TCP peer, when the server was started with connect info. In-process
/// requests (tests, the desktop scheme) have none.
pub struct PeerAddr(pub Option<SocketAddr>);

impl<S: Send + Sync> FromRequestParts<S> for PeerAddr {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts
                .extensions
                .get::<ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
        ))
    }
}
