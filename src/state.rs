use axum::extract::FromRef;
use leptos::prelude::LeptosOptions;
use leptos_axum::AxumRouteListing;

use crate::store::ChallengeStore;

/// Shared axum state for the whole application.
#[derive(FromRef, Debug, Clone)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub store: ChallengeStore,
    pub routes: Vec<AxumRouteListing>,
}
