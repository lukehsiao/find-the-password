use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;
use leptos_axum::AxumRouteListing;
use std::sync::{Arc, Mutex};

use crate::user::{Completion, Users};

#[derive(FromRef, Debug, Clone)]
pub struct Internal {
    pub leptos_options: LeptosOptions,
    pub usermap: Arc<Users>,
    pub leaderboard: Arc<Mutex<Vec<Completion>>>,
    pub routes: Vec<AxumRouteListing>,
}
