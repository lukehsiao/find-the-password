use anyhow::Result;
use tracing_subscriber::EnvFilter;

use challenges::{config::get_config, http::Application};

#[tokio::main]
async fn main() -> Result<()> {
    // Ignore if .env doesn't exist, since we won't use a .env in deployment.
    dotenv::dotenv().ok();

    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "challenges=debug,tower_http=debug")
    }

    // Initialize the tracing-based logs
    tracing_subscriber::fmt()
        // TODO(lukehsiao): at some point, we want json logs sent to a central dashboard.
        // For now, it makes it hard to read.
        // .json()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Grab our configuration parameters
    let config = get_config()?;

    let application = Application::build(config).await?;
    application.run_until_stopped().await?;
    Ok(())
}
