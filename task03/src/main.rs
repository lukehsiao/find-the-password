use std::{
    collections::HashMap,
    include_str,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Extension, Path},
    handler::Handler,
    http::StatusCode,
    response::{Html, Redirect},
    routing::get,
    AddExtensionLayer, Json, Router, Server,
};
use chrono::{DateTime, Local, SecondsFormat};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::{debug, info, instrument};

type SharedState = Arc<RwLock<State>>;

#[derive(Debug)]
struct State {
    users: HashMap<String, UserState>,
    total_hits: u64,
    winners: Vec<(DateTime<Local>, String)>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
struct UserState {
    name: String,
    solved: bool,
    hits_before_solve: u64,
    total_hits: u64,
    #[serde(skip)]
    secret_idx: usize,
    #[serde(skip)]
    passwords: Vec<String>,
}

const NUM_PASSWORDS: usize = 20_000;
const PASS_LEN: usize = 32;
const OFFSET: usize = 4000;

#[instrument]
#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "task03=debug,tower_http=info")
    }
    tracing_subscriber::fmt::init();

    let app = app();

    // Run the server with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    debug!("Listening on {addr}");
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn app() -> Router {
    let shared_state: SharedState = Arc::new(RwLock::new(State {
        users: HashMap::new(),
        total_hits: 0,
        winners: vec![],
    }));

    let app = Router::new()
        .route("/03", get(readme))
        .route(
            "/03/:user",
            get(user_stats)
                .post(create_user)
                .patch(reset_user)
                .delete(del_user),
        )
        .route(
            "/03/:user/passwords.txt",
            get(get_passwords.layer(CompressionLayer::new())),
        )
        .route("/03/:user/check/:password", get(check_password))
        .route("/03/stats", get(get_stats))
        .layer(TraceLayer::new_for_http())
        .layer(AddExtensionLayer::new(shared_state));

    app.fallback(handler_redirect.into_service())
}

/// Provide a catch-all 404 handler.
#[instrument]
async fn handler_redirect() -> Redirect {
    Redirect::permanent("/03".parse().unwrap())
}

/// Provide the README to the root path
#[instrument]
async fn readme() -> Html<&'static str> {
    let readme = include_str!("../README.html");
    Html(readme)
}

/// Get a user-specific list of passwords.
#[instrument]
async fn get_passwords(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Result<String, StatusCode> {
    if let Some(user) = state.read().unwrap().users.get(&username) {
        Ok(user.passwords.join("\n"))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get the current stats for this session
#[instrument]
async fn get_stats(Extension(state): Extension<SharedState>) -> Html<String> {
    let state = state.read().unwrap();

    let mut winner_list = String::new();
    for (datetime, name) in &state.winners {
        let hits_before_solve = state.users.get(name).unwrap().hits_before_solve;
        let entry = format!(
            "<li>[{}] <strong>{}</strong> ({} attempts)</li>\n",
            datetime.to_rfc3339_opts(SecondsFormat::Secs, true),
            name,
            hits_before_solve
        );
        winner_list += &entry;
    }

    let total_hits = state.total_hits;

    let mut players_list = String::new();
    for name in state.users.keys() {
        let entry = format!("<li>{name}</li>\n");
        players_list += &entry;
    }
    Html(format!(
        "
<html lang=\"en\">
    <head><title>Challenge: Current Stats</title>
        <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/water.css@2/out/water.css\">
    </head>

    <body>
        <h1>Current Stats</h1>
        <h2>Leaderboard</h2>
        <ol>
            {winner_list}
        </ol>

        <h2>Registered Players</h2>
        <ul>
            {players_list}
        </ul>
        <strong>Total Attempts:</strong> {total_hits}
    </body>
</html>"
    ))
}

/// Check a password for the given user.
#[instrument]
async fn check_password(
    Path((username, password)): Path<(String, String)>,
    Extension(state): Extension<SharedState>,
) -> Result<String, StatusCode> {
    let mut state = state.write().unwrap();

    if let Some(user) = state.users.get_mut(&username) {
        // Track hits
        if !user.solved {
            user.hits_before_solve += 1;
        }
        user.total_hits += 1;

        // Respond
        let result = match (user.solved, password == user.passwords[user.secret_idx]) {
            (true, true) => Ok("True".to_string()),
            (false, true) => {
                user.solved = true;
                let name = user.name.clone();
                info!(
                    "We have a winner: {}, {}, {}",
                    serde_json::to_string(&user).unwrap(),
                    user.secret_idx,
                    user.passwords[user.secret_idx]
                );
                state.winners.push((Local::now(), name));
                Ok("True".to_string())
            }
            _ => Ok("False".to_string()),
        };
        state.total_hits += 1;
        result
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Delete a user.
///
/// # Example
/// ```
/// curl -X DELETE http://localhost:3000/03/test_user
/// ```
#[instrument]
async fn del_user(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> StatusCode {
    let mut state = state.write().unwrap();
    if let Some(user) = state.users.remove(&username) {
        let winners = &mut state.winners;
        let idx = winners
            .iter()
            .position(|(_, name)| *name == user.name)
            .unwrap();
        winners.remove(idx);
        state.total_hits -= user.total_hits;

        info!("Deleted {}", serde_json::to_string(&user).unwrap(),);

        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Reset a user.
///
/// # Example
/// ```
/// curl -X PATCH http://localhost:3000/03/test_user
/// ```
#[instrument]
async fn reset_user(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> StatusCode {
    let mut rng = rand::thread_rng();
    let secret_idx = rng.gen_range(OFFSET..NUM_PASSWORDS);
    let passwords: Vec<String> = (0..NUM_PASSWORDS)
        .map(|_| {
            rng.clone()
                .sample_iter(&Alphanumeric)
                .take(PASS_LEN)
                .map(char::from)
                .collect()
        })
        .collect();

    let state: &mut State = &mut state.write().unwrap();
    let users = &mut state.users;
    let total_hits = &mut state.total_hits;
    let winners = &mut state.winners;
    if let Some(user) = users.get_mut(&username) {
        // Reset relevant stats
        *total_hits -= user.total_hits;
        if let Some(idx) = winners.iter().position(|(_, name)| *name == user.name) {
            winners.remove(idx);
        }

        user.hits_before_solve = 0;
        user.solved = false;
        user.total_hits = 0;
        user.secret_idx = secret_idx;
        user.passwords = passwords;

        debug!(
            user = %serde_json::to_string(&user).unwrap(),
            secret_idx = %user.secret_idx,
            secret = %user.passwords[user.secret_idx],
            "Reset user."
        );

        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Create a new user.
///
/// # Example
/// ```
/// curl -X POST http://localhost:3000/03/test_user
/// ```
#[instrument]
async fn create_user(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Json<UserState> {
    let mut rng = rand::thread_rng();
    // Don't want it too close to the front for those who will try to brute force
    let secret_idx = rng.gen_range(OFFSET..NUM_PASSWORDS);

    let passwords: Vec<String> = (0..NUM_PASSWORDS)
        .map(|_| {
            rng.clone()
                .sample_iter(&Alphanumeric)
                .take(PASS_LEN)
                .map(char::from)
                .collect()
        })
        .collect();

    let new_user = UserState {
        name: username,
        solved: false,
        hits_before_solve: 0,
        total_hits: 0,
        secret_idx,
        passwords,
    };

    state
        .write()
        .unwrap()
        .users
        .insert(String::from(&new_user.name), new_user.clone());

    debug!(
        "Created new user: {} {} {}",
        serde_json::to_string(&new_user).unwrap(),
        new_user.secret_idx,
        new_user.passwords[new_user.secret_idx],
    );
    Json(new_user)
}

/// Get the stats for a specific user.
#[instrument]
async fn user_stats(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Result<Json<UserState>, (StatusCode, Html<String>)> {
    if let Some(user) = state.read().unwrap().users.get(&username) {
        Ok(Json(user.clone()))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Html(format!(
                "<body><p>There is no user: <strong>{username}</strong></p></body>"
            )),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::{SocketAddr, TcpListener};

    use axum::{
        body::Body,
        http,
        http::{Request, StatusCode},
    };
    use mime;
    use test_log::test;
    use tower::util::ServiceExt;

    #[test(tokio::test)]
    async fn root() {
        let app = app();

        let readme = include_str!("../README.html");

        // `Router` implements `tower::Service<Request<Body>>` so we can
        // call it like any tower service, no need to run an HTTP server.
        let response = app
            .oneshot(Request::builder().uri("/03").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert_eq!(&body[..], readme.as_bytes());
    }

    #[test(tokio::test)]
    async fn create_user() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .method(http::Method::POST)
                    .uri("/03/test_user")
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response_body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let user: UserState = serde_json::from_slice(&response_body).unwrap();
        let gold = UserState {
            name: String::from("test_user"),
            solved: false,
            hits_before_solve: 0,
            total_hits: 0,
            secret_idx: 0,
            passwords: vec![],
        };
        assert_eq!(user, gold);
    }

    #[test(tokio::test)]
    async fn get_password_file() {
        // This test, we need to server to maintain state, so we spawn a real server.
        let listener = TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app().into_make_service())
                .await
                .unwrap()
        });

        let client = hyper::Client::new();

        // First, create a user
        let response = client
            .request(
                Request::builder()
                    .method(http::Method::POST)
                    .uri(format!("http://{addr}/03/test_user"))
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Then, test the passwords file
        let response = client
            .request(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("http://{addr}/03/test_user/passwords.txt"))
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let response_body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        let passwords: Vec<&str> = std::str::from_utf8(&response_body)
            .unwrap()
            .split('\n')
            .collect();

        let first_pass = String::from(passwords[0]);
        assert_eq!(passwords[0].len(), PASS_LEN);

        // Ensure the response is always the same.
        let response = client
            .request(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("http://{addr}/03/test_user/passwords.txt"))
                    .header(http::header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let passwords: Vec<&str> = std::str::from_utf8(&response_body)
            .unwrap()
            .split('\n')
            .collect();

        assert_eq!(&first_pass, passwords[0]);
    }

    #[test(tokio::test)]
    async fn redirect() {
        let app = app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.is_empty());
    }

    #[test(tokio::test)]
    async fn reset_user() {
        // This test, we need to server to maintain state, so we spawn a real server.
        let listener = TcpListener::bind("127.0.0.1:0".parse::<SocketAddr>().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app().into_make_service())
                .await
                .unwrap()
        });
        let client = hyper::Client::new();

        // First, create a user
        let response = client
            .request(
                Request::builder()
                    .method(http::Method::POST)
                    .uri(format!("http://{addr}/03/test_user"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Then, run a few bad checks
        for i in 0..10 {
            let response = client
                .request(
                    Request::builder()
                        .method(http::Method::GET)
                        .uri(format!("http://{addr}/03/test_user/check/{i}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }

        let response = client
            .request(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("http://{addr}/03/stats"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_body = String::from_utf8(
            hyper::body::to_bytes(response.into_body())
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(&response_body.contains("<strong>Total Attempts:</strong> 10"));

        // Then, reset the user
        let response = client
            .request(
                Request::builder()
                    .method(http::Method::PATCH)
                    .uri(format!("http://{addr}/03/test_user"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = client
            .request(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("http://{addr}/03/stats"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let response_body = String::from_utf8(
            hyper::body::to_bytes(response.into_body())
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(&response_body.contains("<strong>Total Attempts:</strong> 0"));
    }
}
