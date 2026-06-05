//! Integration tests for the kid-facing HTTP contract.
//!
//! These drive the real production router (`challenge::router::api_router`) via
//! `tower`'s `oneshot`, so they lock the exact responses players' scripts
//! depend on: literal `true`/`false` bodies, 200/404 statuses, and a
//! byte-for-byte passwords.txt download.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use challenge::{router::api_router, store::ChallengeStore};
use jiff::Timestamp;
use tower::ServiceExt;

/// Build the API router over a store, returning both so tests can seed state
/// through the store and then exercise it over HTTP.
fn app() -> (Router, ChallengeStore) {
    let store = ChallengeStore::new();
    (api_router().with_state(store.clone()), store)
}

async fn get(router: Router, uri: &str) -> (StatusCode, String) {
    let response = router
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn healthcheck_returns_200() {
    let (router, _store) = app();
    let (status, _) = get(router, "/up").await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn check_for_unknown_user_is_404() {
    let (router, _store) = app();
    let (status, _) = get(router, "/u/ghost/check/whatever").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn wrong_password_returns_false() {
    let (router, store) = app();
    store.add_user("alice", Timestamp::now()).unwrap();
    let (status, body) = get(router, "/u/alice/check/definitely-wrong").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "false");
}

#[tokio::test]
async fn correct_password_returns_true() {
    let (router, store) = app();
    store.add_user("bob", Timestamp::now()).unwrap();
    let secret = store.get_user("bob").unwrap().secret;
    let (status, body) = get(router, &format!("/u/bob/check/{secret}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "true");
}

#[tokio::test]
async fn passwords_download_matches_store() {
    let (router, store) = app();
    store.add_user("carol", Timestamp::now()).unwrap();
    let expected = store.passwords("carol").unwrap();
    let (status, body) = get(router, "/u/carol/passwords.txt").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, expected);
    assert_eq!(body.lines().count(), 60_000);
}

#[tokio::test]
async fn passwords_for_unknown_user_is_404() {
    let (router, _store) = app();
    let (status, _) = get(router, "/u/ghost/passwords.txt").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
