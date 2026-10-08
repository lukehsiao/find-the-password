pub mod clock;
pub mod error;
pub mod http;
pub mod listener;
pub mod pages;
pub mod store;
#[cfg(test)]
mod testing;
pub mod user;

use topcoat::{
    context::{Cx, app_context},
    router::{Router, path_param},
};

use crate::{clock::Clock, store::ChallengeStore};

// The `{username}` segment shared by every per-player URL, pages and the
// plain-text routes alike.
path_param!(pub username);

/// The whole application: the pages players click through plus the
/// plain-text routes their scripts hit, all backed by `store` and stamped
/// with `clock`.
///
/// Every route is registered here by hand, so this function is the full
/// URL contract in one place.
pub fn router(store: ChallengeStore, clock: Clock) -> Router {
    Router::builder()
        .layout(pages::layout)
        .page(pages::home)
        .page(pages::join)
        .page(pages::player)
        .page(pages::confirm)
        .route(http::check_password)
        .route(http::passwords_txt)
        .route(http::healthcheck)
        .route(http::robots_txt)
        .route(http::favicon)
        .app_context(store)
        .app_context(clock)
        .build()
}

/// The store every handler reads and writes.
fn store(cx: &Cx) -> &ChallengeStore {
    app_context(cx)
}

/// The clock every handler stamps state changes with.
fn clock(cx: &Cx) -> &Clock {
    app_context(cx)
}
