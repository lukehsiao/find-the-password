use anyhow::Result;
use sqlx::sqlite::SqlitePool;

use task03::{
    config::{get_config, DatabaseConfig},
    http::{get_connection_pool, Application},
};

pub struct TestApp {
    pub addr: String,
    pub port: u16,
    pub db_pool: SqlitePool,
}

async fn configure_database(config: &DatabaseConfig) -> SqlitePool {
    let pool = SqlitePool::connect_with(dbg!(config.with_db()))
        .await
        .expect("Failed to connect to sqlite");

    pool
}

pub async fn spawn_app() -> Result<TestApp> {
    let config = {
        let mut c = get_config().expect("Failed to read configuration.");

        // Just use temporary databases for test
        // See: https://www.sqlite.org/inmemorydb.html
        c.database.uri = "".to_string();

        // Use a random, open OS port
        c.application.port = 0;
        c
    };

    // Create and migrate db
    configure_database(&config.database).await;

    // Launch the application as background task
    let app = Application::build(config.clone())
        .await
        .expect("Failed to build application.");
    let app_port = app.port();
    let _ = tokio::spawn(app.run_until_stopped());

    let test_app = TestApp {
        addr: format!("http://localhost:{}", app_port),
        port: app_port,
        db_pool: get_connection_pool(&config.database).await?,
    };

    Ok(test_app)
}
