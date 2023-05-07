use anyhow::Result;
use pretty_assertions::assert_eq;

use crate::helpers::spawn_app;

#[tokio::test]
async fn healthz_works() -> Result<()> {
    let app = spawn_app().await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(&format!("{}/healthz", &app.addr))
        .send()
        .await
        .expect("Failed to send request");

    assert!(resp.status().is_success());
    assert_eq!(Some(0), resp.content_length());
    Ok(())
}
