use axum::extract::FromRef;
use leptos::LeptosOptions;
use std::sync::{Arc, Mutex};

use crate::user::{Completion, Users};

#[derive(FromRef, Debug, Clone)]
pub struct Internal {
    pub leptos_options: LeptosOptions,
    pub usermap: Arc<Users>,
    pub leaderboard: Arc<Mutex<Vec<Completion>>>,
}
