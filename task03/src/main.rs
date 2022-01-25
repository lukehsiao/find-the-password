use std::{
    collections::{HashMap, HashSet},
    include_str,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{response::Html, routing::get, Router, Server};
use tower_http::trace::TraceLayer;
use tracing::debug;
use tracing_subscriber;

type SharedState = Arc<RwLock<State>>;

#[derive(Debug)]
struct State {
    users: HashMap<String, UserState>,
    total_hits: u64,
    allowed_users: HashSet<String>,
}

#[derive(Debug)]
struct UserState {
    name: String,
    eligible: bool,
    solved: bool,
    hits_before_first_solve: u64,
    secret: String,
}

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "task03=debug,tower_http=debug")
    }
    tracing_subscriber::fmt::init();

    let app = app();

    // Run the server with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    debug!("Listening on {addr}");
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app() -> Router {
    Router::new()
        .route("/03", get(readme))
        .layer(TraceLayer::new_for_http())
}

/// Provide the README to the root path
async fn readme() -> Html<&'static str> {
    let readme = include_str!("../README.html");
    Html(readme)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn hello_world() {
        let app = app();

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], b"Hello, World!");
    }

    #[tokio::test]
    async fn not_found() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.is_empty());
    }
}
