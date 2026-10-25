pub mod clock;
pub mod error;
pub mod http;
pub mod listener;
pub mod pages;
pub mod player;
pub mod store;
#[cfg(test)]
mod testing;

use topcoat::{
    context::{BaseUrl, Cx, app_context},
    router::{Compression, CompressionLevel, Router, path_param},
};

use crate::{clock::Clock, store::ChallengeStore};

// The `{username}` segment shared by every per-player URL, pages and the
// plain-text routes alike.
path_param!(pub username);

/// The whole application: the pages players click through plus the
/// plain-text routes their scripts hit, all backed by `store`, stamped with
/// `clock`, and served to players at `base_url`.
///
/// Every route is registered here by hand, so this function is the full
/// URL contract in one place.
pub fn router(store: ChallengeStore, clock: Clock, base_url: BaseUrl) -> Router {
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
        .base_url(base_url)
        // The only response big enough to matter is passwords.txt, 2MB of
        // random base62 that compresses about 25% at any level. The default
        // level spends 4x the CPU of Fastest on gzip and 2x on brotli, for
        // output at most 1.5% smaller.
        .compression(Compression::new().level(CompressionLevel::Fastest))
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
