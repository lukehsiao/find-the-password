use axum::{Router, extract::FromRef, routing::get};

use crate::{
    http::{check_password, get_passwords, healthcheck},
    store::ChallengeStore,
};

/// The kid-facing HTTP contract plus the healthcheck.
///
/// Generic over the state type so main() can merge it into the leptos app
/// router while integration tests mount it directly on a bare
/// [`ChallengeStore`], exercising the exact production routes without any
/// leptos configuration.
pub fn api_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    ChallengeStore: FromRef<S>,
{
    Router::new()
        .route("/up", get(healthcheck))
        .route("/u/{username}/check/{password}", get(check_password))
        .route("/u/{username}/passwords.txt", get(get_passwords))
}
