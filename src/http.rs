use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jiff::Timestamp;

use crate::{state::AppState, user::Completion};

/// Check a password for correctness.
pub async fn check_password(
    Path((username, password)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Response {
    match state.usermap.get_mut(&username) {
        None => (StatusCode::NOT_FOUND).into_response(),
        Some(mut user) => {
            user.hits_before_solved += 1;
            if user.check_password(&password) {
                user.solved_at = Some(Timestamp::now());
                // Solved! Add to leaderboard.
                (*state.leaderboard).lock().unwrap().push(Completion {
                    username: user.username.clone(),
                    time_to_solve: user.solved_at.unwrap() - user.created_at,
                    attempts_to_solve: user.hits_before_solved,
                });

                (StatusCode::OK, "true").into_response()
            } else {
                (StatusCode::OK, "false").into_response()
            }
        }
    }
}

/// Produce passwords.txt for a suer.
pub async fn get_passwords(
    Path(username): Path<String>,
    State(state): State<AppState>,
) -> Response {
    match state.usermap.get_mut(&username) {
        None => (StatusCode::NOT_FOUND).into_response(),
        Some(user) => (StatusCode::OK, user.get_passwords()).into_response(),
    }
}
