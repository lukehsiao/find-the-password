use std::{collections::HashMap, env, include_str, net::SocketAddr};

use anyhow::{Context, Result};
use axum::{
    body::Bytes,
    extract::{ContentLengthLimit, Extension},
    handler::Handler,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router, Server,
};
use lazy_static::lazy_static;
use sqlx::SqlitePool;
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
async fn main() -> Result<()> {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "task04=debug,tower_http=info")
    }
    if std::env::var_os("DATABASE_URL").is_none() {
        std::env::set_var("DATABASE_URL", "sqlite:task04.db")
    }
    tracing_subscriber::fmt::init();

    let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;

    sqlx::migrate!()
        .run(&pool)
        .await
        .context("Failed to migrate database")?;

    debug!("Migrated the database.");

    let app = app(pool);

    // Run the server with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
    debug!("Listening on {addr}");

    Ok(())
}

fn app(pool: SqlitePool) -> Router {
    let app = Router::new()
        .route("/04", get(readme))
        .route("/04/files", get(download.layer(CompressionLayer::new())))
        .route("/04/upload", post(check_upload))
        .layer(Extension(pool))
        .layer(TraceLayer::new_for_http());

    app.fallback(handler_redirect.into_service())
}

/// Provide a catch-all 404 handler.
async fn handler_redirect() -> Redirect {
    Redirect::permanent("/03")
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
    ContentLengthLimit(_bytes): ContentLengthLimit<Bytes, { 1024 * 1_000 }>,
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
    async fn check_upload() -> Result<()> {
        let pool = SqlitePool::connect(&env::var("DATABASE_URL")?).await?;
        let app = app(pool);

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
        Ok(())
    }
}
