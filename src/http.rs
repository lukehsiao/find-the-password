use axum::{
    extract::{Path, State},
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
    Path((username, password)): Path<(String, String)>,
    State(store): State<ChallengeStore>,
) -> Response {
    match store.check(&username, &password, Timestamp::now()) {
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
