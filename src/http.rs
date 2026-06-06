use axum::{
    extract::{Path, RawPathParams, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jiff::Timestamp;

use crate::store::{ChallengeStore, CheckOutcome};

/// Simple healthcheck endpoint.
pub async fn healthcheck() -> impl IntoResponse {
    StatusCode::OK
}

/// Check a password for correctness.
///
/// The literal `true`/`false` bodies and the 200/404 statuses are the
/// contract that players' scripts depend on.
pub async fn check_password(
    params: RawPathParams,
    State(store): State<ChallengeStore>,
) -> Response {
    // RawPathParams borrows the captures the router already decoded,
    // skipping the two String allocations Path<(String, String)> would
    // make on the hottest route in the app.
    let mut username = None;
    let mut password = None;
    for (name, value) in &params {
        match name {
            "username" => username = Some(value),
            "password" => password = Some(value),
            _ => {}
        }
    }
    let (Some(username), Some(password)) = (username, password) else {
        // The route template guarantees both captures; stay graceful anyway.
        return StatusCode::NOT_FOUND.into_response();
    };
    match store.check(username, password, Timestamp::now) {
        CheckOutcome::NotFound => StatusCode::NOT_FOUND.into_response(),
        CheckOutcome::Incorrect => (StatusCode::OK, "false").into_response(),
        CheckOutcome::Correct => (StatusCode::OK, "true").into_response(),
    }
}

/// Produce passwords.txt for a user.
pub async fn get_passwords(
    Path(username): Path<String>,
    State(store): State<ChallengeStore>,
) -> Response {
    match store.passwords(&username) {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(passwords) => (StatusCode::OK, passwords).into_response(),
    }
}
