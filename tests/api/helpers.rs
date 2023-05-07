use anyhow::Result;
use once_cell::sync::Lazy;
use sqlx::sqlite::SqlitePool;

use challenges::{
    config::{get_config, DatabaseConfig},
    http::{get_connection_pool, Application},
    telemetry::{get_subscriber, init_subscriber},
};

// Ensure that the `tracing` stack is only initialised once using `once_cell`
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(subscriber_name, default_filter_level, std::io::sink);
        init_subscriber(subscriber);
    };
});

pub struct TestApp {
    pub addr: String,
    pub port: u16,
    pub db_pool: SqlitePool,
}

async fn configure_database(config: &DatabaseConfig) -> SqlitePool {
    let connection_pool = SqlitePool::connect_with(dbg!(config.with_db()))
        .await
        .expect("Failed to connect to sqlite");
    // Migrate database
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}

pub async fn spawn_app() -> Result<TestApp> {
    // Initialize telemetry
    Lazy::force(&TRACING);

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
