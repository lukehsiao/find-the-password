use std::{
    collections::HashMap,
    include_str,
    io::{self, Write},
    net::SocketAddr,
    process::Command,
};

use axum::{
    body::Bytes,
    extract::ContentLengthLimit,
    handler::Handler,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router, Server,
};
use lazy_static::lazy_static;
use tempfile::tempfile;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::debug;

lazy_static! {
    static ref HASHMAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        let hashes = include_str!("../sha256sum.txt");

        for line in hashes.lines() {
            let row: Vec<&str> = line.split_whitespace().collect();
            m.insert(row[1], row[0]);
        }
        m
    };
}

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "task04=debug,tower_http=info")
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
    let app = Router::new()
        .route("/04", get(readme))
        .route("/04/files", get(download.layer(CompressionLayer::new())))
        .route("/04/upload", post(check_upload))
        .layer(TraceLayer::new_for_http());

    app.fallback(handler_redirect.into_service())
}

/// Provide a catch-all 404 handler.
async fn handler_redirect() -> Redirect {
    Redirect::permanent("/03".parse().unwrap())
}

/// Provide the README to the root path
async fn readme() -> Html<&'static str> {
    let readme = include_str!("../README.html");
    Html(readme)
}

async fn download() -> impl IntoResponse {
    StatusCode::OK
}

// Take in only 1mb
async fn check_upload(
    ContentLengthLimit(bytes): ContentLengthLimit<Bytes, { 1024 * 1_000 }>,
) -> impl IntoResponse {
    StatusCode::OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use test_log::test;
    use tower::util::ServiceExt;

    #[test(tokio::test)]
    async fn check_upload() {
        let app = app();

        let readme = include_str!("../README.html");

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(Request::builder().uri("/03").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], readme.as_bytes());
    }
}
