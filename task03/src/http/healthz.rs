//! Contains the routes and API implementation for the health check.
use axum::{
    http::StatusCode,
    routing::{get, Router},
};

pub fn router() -> Router {
    // Each module is responsible for setting up its own routing, making the root module a lot
    // cleaner.
    Router::new().route("/healthz", get(healthcheck))
}

pub async fn healthcheck() -> StatusCode {
    StatusCode::OK
}
