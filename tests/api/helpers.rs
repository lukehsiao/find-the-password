use std::fs;

use anyhow::Result;
use once_cell::sync::Lazy;
use sqlx::sqlite::SqlitePool;
use tokio::runtime::Runtime;
use uuid::Uuid;

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
    pub api_client: reqwest::Client,
    pub db_name: String,
}

impl TestApp {
    pub async fn post_user(&self, user: String) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/03/u/{}", &self.addr, user))
            .send()
            .await
            .expect("Failed to execute request.")
    }
    pub async fn delete_user(&self, user: String) -> reqwest::Response {
        self.api_client
            .delete(&format!("{}/03/u/{}", &self.addr, user))
            .send()
            .await
            .expect("Failed to execute request.")
    }
}

async fn configure_database(config: &DatabaseConfig) -> SqlitePool {
    let connection_pool = SqlitePool::connect_with(config.with_db())
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

        // Use a unique, random database for each test case so they can go in parallel
        c.database.uri = format!("{}.db", Uuid::new_v4());

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

    let api_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    let test_app = TestApp {
        addr: format!("http://localhost:{}", app_port),
        port: app_port,
        db_pool: get_connection_pool(&config.database).await?,
        api_client,
        db_name: config.database.uri,
    };

    Ok(test_app)
}

// Test tear down happens on drop, which will happen even if a test fails or panics mid-way.
impl Drop for TestApp {
    fn drop(&mut self) {
        // This is a hacky workaround to calling async code from this sync Drop trait to clean up.
        let (tx, rx) = std::sync::mpsc::channel();
        let db_name = self.db_name.clone();

        std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                fs::remove_file(db_name).expect("Failed to delete the sqlite db");
                let _ = tx.send(());
            })
        });

        let _ = rx.recv();
    }
}
