use axum::extract::FromRef;
use leptos::LeptosOptions;
use std::sync::Arc;

use crate::user::{Completion, UserMap};

#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub usermap: Arc<UserMap>,
    pub leaderboard: Arc<Vec<Completion>>,
}
