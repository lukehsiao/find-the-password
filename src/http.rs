//! Contains the setup code for the API built with Axum.
//!
//! The API routes and implementions exist in child modules of this.

use std::{future::Future, net::TcpListener, pin::Pin};

use anyhow::Result;
use axum::Router;
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    request_id::MakeRequestUuid,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    ServiceBuilderExt,
};
use tracing::{info, Level};

use crate::config::{Config, DatabaseConfig};

// Modules introducing API routes
pub mod extractors;
pub mod healthz;
pub mod passwords;

pub struct Application {
    port: u16,
    server: Pin<Box<dyn Future<Output = hyper::Result<()>> + Send>>,
}

impl Application {
    pub async fn build(config: Config) -> Result<Self> {
        let connection_pool = get_connection_pool(&config.database).await?;

        let addr = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(&addr)?;
        info!("Listening on {}", addr);
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, connection_pool).await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), hyper::Error> {
        self.server.await
    }
}

pub async fn run(
    listener: TcpListener,
    db_pool: SqlitePool,
) -> Result<Pin<Box<dyn Future<Output = hyper::Result<()>> + Send>>> {
    // build our application with some routes
    let app = api_router(&db_pool).layer(
        ServiceBuilder::new()
            .set_x_request_id(MakeRequestUuid)
            // log requests and responses
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(
                        DefaultMakeSpan::new()
                            .include_headers(true)
                            .level(Level::INFO),
                    )
                    .on_response(
                        DefaultOnResponse::new()
                            .include_headers(true)
                            .level(Level::INFO),
                    ),
            )
            // propagate the header to the response before the response reaches `TraceLayer`
            .propagate_x_request_id(),
    );

    Ok(Box::pin(
        axum::Server::from_tcp(listener)?
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_handler()),
    ))
}

/// Helper function for generating our router.
///
/// This makes it easier to unit test the route handlers.
fn api_router(state: &SqlitePool) -> Router {
    // This just follows the order that the modules were authored in, order doesn't matter.
    healthz::router().merge(passwords::router(state))
}

// Want to have a graceful shutdown.
async fn shutdown_handler() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    info!("signal received, starting graceful shutdown");
}

/// Utility to grab a connection pool
pub async fn get_connection_pool(config: &DatabaseConfig) -> Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_lazy_with(config.with_db());

    info!("Connected to {:?}", &config);

    Ok(pool)
}
