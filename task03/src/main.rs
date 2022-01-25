use std::{
    collections::{HashMap, HashSet},
    include_str,
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Extension, Path},
    http::StatusCode,
    response::Html,
    routing::get,
    AddExtensionLayer, Json, Router, Server,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing::debug;

type SharedState = Arc<RwLock<State>>;

#[derive(Debug)]
struct State {
    users: HashMap<String, UserState>,
    total_hits: u64,
    allowed_users: HashSet<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
struct UserState {
    name: String,
    eligible: bool,
    solved: bool,
    hits_before_first_solve: u64,
    #[serde(skip)]
    secret_idx: usize,
    #[serde(skip)]
    passwords: Vec<String>,
}

const NUM_PASSWORDS: usize = 20_000;

const PASS_LEN: usize = 32;

#[tokio::main]
async fn main() {
    // Set the RUST_LOG, if it hasn't been explicitly defined
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "task03=debug,tower_http=debug")
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
    let mut allowed_users: HashSet<String> = HashSet::new();
    allowed_users.insert("test_user".to_string());

    let shared_state: SharedState = Arc::new(RwLock::new(State {
        users: HashMap::new(),
        total_hits: 0,
        allowed_users,
    }));

    Router::new()
        .route("/03", get(readme))
        .route("/03/:user", get(user_stats).post(create_user))
        .route("/03/:user/passwords.txt", get(get_passwords))
        .layer(TraceLayer::new_for_http())
        .layer(AddExtensionLayer::new(shared_state))
}

/// Provide the README to the root path
async fn readme() -> Html<&'static str> {
    let readme = include_str!("../README.html");
    Html(readme)
}

/// Get a user-specific list of passwords.
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

/// Create a new user.
///
/// # Example
/// ```
/// curl -X POST http://localhost:3000/03/test_user
/// ```
async fn create_user(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Result<Json<UserState>, StatusCode> {
    if state.read().unwrap().allowed_users.contains(&username) {
        let mut rng = rand::thread_rng();
        // Don't want it too close to the front for those who will try to brute force
        let secret_idx = rng.gen_range(3000..NUM_PASSWORDS);

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
            eligible: true,
            solved: false,
            hits_before_first_solve: 0,
            secret_idx,
            passwords,
        };

        state
            .write()
            .unwrap()
            .users
            .insert(String::from(&new_user.name), new_user.clone());

        debug!(
            user = %serde_json::to_string(&new_user).unwrap(),
            secret_idx = %new_user.secret_idx,
            secret = %new_user.passwords[new_user.secret_idx],
            "Created new user"
        );
        Ok(Json(new_user))
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Get the stats for a specific user.
async fn user_stats(
    Path(username): Path<String>,
    Extension(state): Extension<SharedState>,
) -> Result<Json<UserState>, StatusCode> {
    if let Some(user) = state.read().unwrap().users.get(&username) {
        Ok(Json(user.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
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
    use tower::util::ServiceExt;

    #[tokio::test]
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

    #[tokio::test]
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
            eligible: true,
            solved: false,
            hits_before_first_solve: 0,
            secret_idx: 0,
            passwords: vec![],
        };
        assert_eq!(user, gold);
    }

    #[tokio::test]
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

    #[tokio::test]
    async fn not_found() {
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

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.is_empty());
    }
}
