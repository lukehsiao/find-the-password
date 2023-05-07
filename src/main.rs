use anyhow::Result;

use challenges::{
    config::get_config,
    http::Application,
    telemetry::{get_subscriber, init_subscriber},
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize telemetry
    let subscriber = get_subscriber("challenges".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    // Grab our configuration parameters
    let config = get_config()?;

    let application = Application::build(config).await?;
    application.run_until_stopped().await?;
    Ok(())
}
