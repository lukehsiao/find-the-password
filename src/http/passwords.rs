//! Contains the routes and API implementation for the password challenge.
//!
//! The goal of this challenge is to learn simple brute-force automation
//! running against a real web server.
//!
//! ## How to Play
//!
//! The list of passwords is available at:
//! ```text
//! https://challenge.hsiao.dev/03/<name>/passwords.txt
//! ```
//!
//! Where `<name>` is your username (e.g., `alexh`).
//!
//! You can check if a password is the one I lost by checking the website with it in the URL following this template:
//! ```text
//! https://challenge.hsiao.dev/03/<name>/check/<password>
//! ```
//!
//! For example, if I wanted to test the password: `testpass`, I would visit
//! ```text
//! https://challenge.hsiao.dev/03/luke/check/testpass
//! ```
//!
//! And I’d see the response:
//!
//! ```text
//! False
//! ```
//!
//! If I get the right password, I’d see:
//!
//! ```text
//! True
//! ```
//!
//! You can check some stats about everyone's attempts by visiting
//!
//! ```text
//! https://challenge.hsiao.dev/03/status
//! ```
//!
//! ## Rules
//!
//! - No sharing a solution with each other, everyone has to do their own work, but you’re free to collaborate!
//! - If you can solve it, you have to share with me what you did!
//! - Parents are not allowed to help much. I’ll leave it to parents' judgement on what “much” is. When in doubt, feel free to send them to me!
//! - Only use the url with your own name in it, don’t impersonate others!
//! - There is no limit to how many times you can try!
//! - I will update this email thread as prizes are claimed!
//!
//! ## Some solutions
//!
//! Brute force is the only answer.
//!
//! - Ideally, this is a trivial for loop over the passwords and making a web requests, checking for
//! "Yes" in the response.
//! - Turns out you can also use a spreadsheet, leveraging something like `=WEBSERVICE` to make the web
//! requests for you. Turns out Google Sheet's `IMPORTDATA` only allows 50 per sheet, so no go there.
//! - Some kids actually did brute force, turns out 10k wasn't crazy enough! Their approach was to use a
//! multiple tab opener and literally open hundreds of Chrome tabs, closing them quickly as they kept
//! their eyes trained on where "No" and "Yes" were displayed.
//!
//! ## Running the Parallelized Rust Example Solution
//!
//! ```bash
//! $ curl -L https://challenge.hsiao.dev/03/luke/passwords.txt | cargo run --release --example=solution --
//! ```
//!
//! _This challenge was inspired by Marc Scott's blog post: [Kids can't use computers... and this is why
//! it should worry you](http://www.coding2learn.org/blog/2013/07/29/kids-cant-use-computers/)._
//!
//! > When we teach kids to ride a bike, at some point we have to take the training wheels off. Here's
//! > an idea. When they hit eleven, give them a plaintext file with ten-thousand WPA2 keys and tell
//! > them that the real one is in there somewhere. See how quickly they discover Python or Bash then.
use std::include_str;

use anyhow::Context;
use axum::{
    extract::Path,
    handler::Handler,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post, Router},
    Json,
};
use chrono::Utc;
use rand::{distributions::Alphanumeric, rngs::StdRng, Rng, SeedableRng};
use serde::Serialize;
use sqlx::{
    sqlite::{Sqlite, SqlitePool},
    Acquire, FromRow, Transaction,
};
use tower_http::compression::CompressionLayer;
use tracing::info;
use uuid::Uuid;

use crate::http::extractors::DatabaseConnection;

#[derive(thiserror::Error)]
pub enum UserError {
    #[error("A user with username={0} already exists.")]
    AlreadyExists(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }
    Ok(())
}
impl std::fmt::Debug for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl IntoResponse for UserError {
    fn into_response(self) -> Response {
        match self {
            UserError::AlreadyExists(_) => {
                (StatusCode::BAD_REQUEST, self.to_string()).into_response()
            }
            UserError::UnexpectedError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", self)).into_response()
            }
        }
    }
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct UserState {
    #[serde(skip)]
    pub user_id: String,
    pub username: String,
    pub created_at: String,
    pub solved: i64,
    pub solved_at: Option<String>,
    pub hits_before_solved: i64,
    pub total_hits: i64,
    #[serde(skip)]
    pub seed: i64,
    #[serde(skip)]
    pub secret: String,
}

pub const NUM_PASSWORDS: usize = 50_000;
const PASS_LEN: usize = 32;
const OFFSET: usize = 10_000;

pub fn router(state: &SqlitePool) -> Router {
    // Each module is responsible for setting up its own routing, making the root module a lot
    // cleaner.
    Router::new()
        .route("/03", get(readme))
        .route("/03/u/:user", post(create_user).delete(del_user))
        .route(
            "/03/u/:user/passwords.txt",
            get(get_passwords.layer(CompressionLayer::new())),
        )
        .route("/03/u/:user/check/:password", get(check_password))
        .with_state(state.clone())
}

/// Provide the README to the root path
#[tracing::instrument(name = "Getting the homepage", skip(_conn))]
async fn readme(DatabaseConnection(_conn): DatabaseConnection) -> Html<&'static str> {
    let readme = include_str!("../../static/README.html");
    Html(readme)
}

/// Create a new user.
///
/// # Example
/// ```
/// curl -X POST http://localhost:3000/03/u/test_user
/// ```
#[tracing::instrument(name = "Adding a new user", skip(conn))]
async fn create_user(
    Path(username): Path<String>,
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<Json<String>, UserError> {
    let seed: i64 = username
        .as_bytes()
        .iter()
        .map(|x| i64::from(*x))
        .sum::<i64>()
        + Utc::now().timestamp_millis();

    let rng = StdRng::seed_from_u64(seed as u64);

    let secret: String = rng
        .sample_iter(&Alphanumeric)
        .take(PASS_LEN)
        .map(char::from)
        .collect();

    let user_id = Uuid::new_v4().to_string();

    let mut conn = conn;

    // Start a transaction, so that everything happens together.
    let mut transaction = conn
        .begin()
        .await
        .context("Failed to acquire a database connection from the pool")?;

    let now = Utc::now().to_rfc3339();
    match sqlx::query!(
        r#"
    INSERT INTO user (
        user_id,
        username,
        created_at,
        solved,
        hits_before_solved,
        total_hits,
        seed,
        secret
    )
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        user_id,
        username,
        now,
        false,
        0,
        0,
        seed,
        secret
    )
    .execute(&mut transaction)
    .await
    {
        Ok(_) => (),
        Err(e) => {
            if let sqlx::Error::Database(database_error) = e {
                // Match on specific error types
                if let Some(v) = database_error.code() {
                    if v == "2067" {
                        return Err(UserError::AlreadyExists(username));
                    } else {
                        return Err(database_error).context("Failed SQLx query.")?;
                    }
                }
            } else {
                return Err(e).context("Failed SQLx query.")?;
            }
        }
    }

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to add a new user.")?;

    Ok(Json(user_id))
}

#[tracing::instrument(
    name = "Updating user details in the database",
    skip(user, transaction)
)]
async fn update_user(
    transaction: &mut Transaction<'_, Sqlite>,
    user: &UserState,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
    UPDATE user SET
        solved = $1,
        solved_at = $2,
        hits_before_solved = $3,
        total_hits = $4
    WHERE user_id = $5
            "#,
        user.solved,
        user.solved_at,
        user.hits_before_solved,
        user.total_hits,
        user.user_id
    )
    .execute(transaction)
    .await?;

    Ok(())
}

/// Delete a user.
///
/// # Example
/// ```
/// curl -X DELETE http://localhost:3000/03/u/test_user
/// ```
#[tracing::instrument(name = "Deleting the user", skip(conn))]
async fn del_user(
    Path(username): Path<String>,
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<StatusCode, UserError> {
    let mut conn = conn;

    // Start a transaction, so that everything happens together.
    let mut transaction = conn
        .begin()
        .await
        .context("Failed to acquire a database connection from the pool")?;

    let result = sqlx::query!("DELETE FROM user WHERE username = ?", username)
        .execute(&mut transaction)
        .await
        .context("Failed SQLx query.")?;

    // Return 404 if it's an invalid username
    if result.rows_affected() == 0 {
        return Ok(StatusCode::NOT_FOUND);
    }

    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to add a new user.")?;

    Ok(StatusCode::OK)
}

/// Get a user-specific list of passwords.
#[tracing::instrument(name = "Generating passwords for the user", skip(conn))]
async fn get_passwords(
    Path(username): Path<String>,
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<String, StatusCode> {
    let mut conn = conn;

    if let Some(user) =
        sqlx::query_as!(UserState, "SELECT * FROM user WHERE username = ?", username)
            .fetch_optional(&mut conn)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let mut rng = StdRng::seed_from_u64(user.seed as u64);

        let mut passwords: Vec<String> = (0..NUM_PASSWORDS)
            .map(|_| {
                (&mut rng)
                    .sample_iter(&Alphanumeric)
                    .take(PASS_LEN)
                    .map(char::from)
                    .collect()
            })
            .collect();

        // The first password generated is the secret, so swap it later into the list.
        let offset = user.seed as usize % (NUM_PASSWORDS - OFFSET) + OFFSET;
        passwords.swap(0, offset);

        info!(
            user = %serde_json::to_string(&user).unwrap(),
            offset = offset,
            "Generated {NUM_PASSWORDS} passwords"
        );

        Ok(passwords.join("\n"))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
/// Check a password for a given user.
#[tracing::instrument(name = "Checking password for the user", skip(conn))]
async fn check_password(
    Path((username, password)): Path<(String, String)>,
    DatabaseConnection(conn): DatabaseConnection,
) -> Result<String, StatusCode> {
    let mut conn = conn;

    // Start a transaction, so that everything happens together.
    let mut transaction = conn
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(mut user) =
        sqlx::query_as!(UserState, "SELECT * FROM user WHERE username = ?", username)
            .fetch_optional(&mut transaction)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        // Track hits
        if user.solved == 0 {
            user.hits_before_solved += 1;
        }
        user.total_hits += 1;

        // Respond
        let result = match (user.solved, password == user.secret) {
            (1, true) => Ok("True".to_string()),
            (0, true) => {
                user.solved = 1;
                user.solved_at = Some(Utc::now().to_rfc3339());
                info!(
                    user = %serde_json::to_string(&user).unwrap(),
                    secret = %user.secret,
                    "We have a winner!",
                );
                Ok("True".to_string())
            }
            _ => Ok("False".to_string()),
        };

        update_user(&mut transaction, &user)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        transaction
            .commit()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        result
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
