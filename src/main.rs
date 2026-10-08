use std::{env, io};

use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use challenge::{listener::ChallengeListener, router, store::ChallengeStore};

// Per-request allocations live mostly in hyper and the topcoat router;
// mimalloc serves those faster than the system allocator.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> io::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "challenge=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // HOST and PORT, with the same defaults as `topcoat::start`, which is
    // also what `topcoat dev` sets for the app it supervises.
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
    let port: u16 = match env::var("PORT") {
        Ok(port) => port.parse().map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid PORT {port:?}: {err}"),
            )
        })?,
        Err(_) => 3000,
    };

    let listener = ChallengeListener::bind((host.as_str(), port)).await?;
    info!("listening on http://{}", listener.local_addr()?);
    // Ctrl+C or SIGTERM drains in-flight requests before exiting.
    topcoat::serve(listener, router(ChallengeStore::new())).await
}
