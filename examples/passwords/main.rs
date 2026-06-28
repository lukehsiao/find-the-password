use std::{
    io::{self, Read},
    process, thread,
};

use anyhow::{Result, anyhow, ensure};
use futures::{StreamExt, stream};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::ClientBuilder;
use tracing::{debug, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let urls: Vec<String> = input
        .lines()
        .map(|pass| format!("https://challenge.hsiao.dev/u/example/check/{pass}"))
        .collect();

    let client = ClientBuilder::new().build()?;

    let num_cpus = thread::available_parallelism().map_or(1, std::num::NonZero::get);
    info!("Num CPUs: {num_cpus}");

    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, ETA {eta})",
        )?
        .progress_chars("#>-"),
    );

    let bodies = stream::iter(urls)
        .map(|url| {
            let client = &client;
            let pass = url.rsplit('/').next().unwrap().to_string();
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
            pb.inc(1);
            match b {
                Ok((pass, body)) if &body == "true" => {
                    pb.finish_and_clear();
                    println!("Password is: {pass}");
                    process::exit(0);
                }
                Ok((pass, body)) => {
                    debug!("{pass}: {body}");
                }
                _ => {}
            }
        })
        .await;

    pb.finish_and_clear();
    Err(anyhow!("Didn't find the password :("))
}
