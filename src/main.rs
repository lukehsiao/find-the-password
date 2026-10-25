use std::{env, io};

use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use challenge::{clock::Clock, listener::ChallengeListener, router, store::ChallengeStore};
use topcoat::context::BaseUrl;

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

    // Where players reach the server, which the instructions spell out in
    // full. Behind a proxy it differs from the bind address, so it is its
    // own setting.
    let base_url = env::var("BASE_URL").unwrap_or_else(|_| format!("http://localhost:{port}"));
    let invalid = |reason: &dyn std::fmt::Display| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid BASE_URL {base_url:?}: {reason}"),
        )
    };
    let base_url = BaseUrl::new(&base_url).map_err(|err| invalid(&err))?;
    // Routes are served from `/`, and redirects and links stay
    // root-relative, so a path prefix would only half work.
    if !base_url.prefix().is_empty() {
        return Err(invalid(&"it must not have a path"));
    }

    let listener = ChallengeListener::bind((host.as_str(), port)).await?;
    info!("listening on http://{}", listener.local_addr()?);
    let app = router(ChallengeStore::new(), Clock::system(), base_url);
    // Ctrl+C or SIGTERM drains in-flight requests before exiting.
    topcoat::serve(listener, app).await
}
