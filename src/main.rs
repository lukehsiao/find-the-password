use axum::{
    body::Body as AxumBody,
    extract::State,
    http::Request,
    response::{IntoResponse, Response},
};
use leptos::prelude::provide_context;

use challenge::{app::shell, state::AppState};

async fn server_fn_handler(
    State(app_state): State<AppState>,
    request: Request<AxumBody>,
) -> impl IntoResponse {
    use leptos_axum::handle_server_fns_with_context;
    handle_server_fns_with_context(
        move || {
            provide_context(app_state.store.clone());
        },
        request,
    )
    .await
}

async fn leptos_routes_handler(state: State<AppState>, req: Request<AxumBody>) -> Response {
    let State(app_state) = state.clone();
    let handler = leptos_axum::render_route_with_context(
        app_state.routes.clone(),
        move || {
            provide_context(app_state.store.clone());
        },
        move || shell(app_state.leptos_options.clone()),
    );
    handler(state, req).await.into_response()
}

#[tokio::main]
async fn main() {
    use axum::{Router, routing, serve::ListenerExt};
    use leptos::prelude::*;
    use leptos_axum::{LeptosRoutes, generate_route_list};
    use tower_http::compression::CompressionLayer;
    use tracing::info;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    use challenge::{
        app::App,
        router::{check_router, passwords_router},
        state::AppState,
        store::ChallengeStore,
    };

    // Enable tracing.
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "challenge=debug,tower_http=debug,axum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setting get_configuration(None) means we'll be using cargo-leptos's env values
    // For deployment these variables are:
    // <https://github.com/leptos-rs/start-axum#executing-a-server-on-a-remote-machine-without-the-toolchain>
    // Alternately a file can be specified such as Some("Cargo.toml")
    // The file would need to be included with the executable when moved to deployment
    let conf = get_configuration(None).unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);
    let app_state = AppState {
        leptos_options,
        store: ChallengeStore::new(),
        routes: routes.clone(),
    };

    // build our application with a route
    let app = Router::new()
        .merge(passwords_router())
        .route(
            "/api/{*fn_name}",
            routing::get(server_fn_handler).post(server_fn_handler),
        )
        .leptos_routes_with_handler(routes, routing::get(leptos_routes_handler))
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .layer(CompressionLayer::new())
        // Router::layer only wraps the routes added above it, so the check
        // route merged here skips the compression machinery; its 4-5 byte
        // bodies were never compressible anyway.
        .merge(check_router())
        .with_state(app_state);

    // Nagle would sit on the tiny check responses waiting for data that is
    // never coming; every response here fits in one write.
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap()
        .tap_io(|tcp| {
            if let Err(err) = tcp.set_nodelay(true) {
                tracing::debug!("failed to set TCP_NODELAY: {err}");
            }
        });
    info!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

#[cfg(not(feature = "ssr"))]
pub fn main() {
    // no client-side main function
    // unless we want this to work with e.g., Trunk for a purely client-side app
    // see lib.rs for hydration function instead
}

#[cfg(feature = "ssr")]
async fn shutdown_signal() {
    use tokio::signal;
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
