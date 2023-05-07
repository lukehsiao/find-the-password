//! Contains the setup code for the API built with Axum.
//!
//! The API routes and implementions exist in child modules of this.

use std::{future::Future, net::TcpListener, pin::Pin};

use anyhow::Result;
use axum::{extract::Extension, Router};
use sqlx::sqlite::SqlitePool;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::{debug, info};

use crate::config::{Config, DatabaseConfig};

// Modules introducing API routes
pub mod healthz;

pub struct Application {
    port: u16,
    server: Pin<Box<dyn Future<Output = hyper::Result<()>> + Send>>,
}

impl Application {
    pub async fn build(config: Config) -> Result<Self> {
        let connection_pool = get_connection_pool(&config.database).await?;

        // Embeds database migrations in the application binary so we ensure the database is
        // migrated correctly on startup.
        sqlx::migrate!()
            .run(&connection_pool)
            .await
            .expect("Failed to migrate the database");

        let addr = format!("{}:{}", config.application.host, config.application.port);
        let listener = TcpListener::bind(&addr)?;
        debug!("Listening on {}", addr);
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
    let app = api_router()
        .layer(TraceLayer::new_for_http())
        .layer(Extension(db_pool));

    Ok(Box::pin(
        axum::Server::from_tcp(listener)?
            .serve(app.into_make_service())
            .with_graceful_shutdown(shutdown_handler()),
    ))
}

/// Helper function for generating our router.
///
/// This makes it easier to unit test the route handlers.
fn api_router() -> Router {
    // This just follows the order that the modules were authored in, order doesn't matter.
    healthz::router()
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
    let pool = SqlitePool::connect_lazy_with(config.with_db());

    debug!("Connected to {:?}", &config);

    Ok(pool)
}
