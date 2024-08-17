use axum::extract::{Path, State};
use http::StatusCode;
use jiff::Timestamp;

use crate::{state::AppState, user::Completion};

/// Check a password for correctness.
pub async fn check_password(
    Path((username, password)): Path<(String, String)>,
    State(state): State<AppState>,
) -> Result<String, StatusCode> {
    dbg!(&state);
    match state.usermap.get_mut(&username) {
        None => Ok("false".to_string()),
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

                Ok("true".to_string())
            } else {
                Ok("false".to_string())
            }
        }
    }
}
