use std::sync::{Arc, LazyLock, Mutex};

use dashmap::{DashMap, Entry};
use jiff::Timestamp;
use regex::Regex;
use tracing::info;

use crate::{
    error::AppError,
    user::{AttemptResult, Completion, User},
};

/// The outcome of checking one password guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    NotFound,
    Incorrect,
    Correct,
}

/// In-memory store of all challenge state.
///
/// This is the one interface over the user map and the leaderboard; handlers
/// and server functions stay thin adapters over it. Cloning is cheap (two
/// `Arc`s), and constructing one is the entire test setup. Timestamps are
/// passed in by callers so the store itself never touches the clock.
#[derive(Debug, Clone, Default)]
pub struct ChallengeStore {
    users: Arc<DashMap<String, User>>,
    leaderboard: Arc<Mutex<Vec<Completion>>>,
}

impl ChallengeStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate and register a new user.
    ///
    /// # Errors
    /// - [`AppError::InvalidUsername`] if the name fails [`valid_username`].
    /// - [`AppError::UsernameTaken`] if the name already exists.
    pub fn add_user(&self, username: &str, now: Timestamp) -> Result<(), AppError> {
        if !valid_username(username) {
            return Err(AppError::InvalidUsername);
        }
        // The entry API reserves the slot atomically, so two concurrent
        // requests for the same name cannot both succeed.
        match self.users.entry(username.to_owned()) {
            Entry::Occupied(_) => Err(AppError::UsernameTaken),
            Entry::Vacant(slot) => {
                slot.insert(User::new(username.to_owned(), now));
                info!(username, "added user");
                Ok(())
            }
        }
    }

    /// Snapshot of a user, if present.
    #[must_use]
    pub fn get_user(&self, username: &str) -> Option<User> {
        self.users.get(username).map(|user| user.clone())
    }

    /// Generate the deterministic password file for a user, if present.
    #[must_use]
    pub fn passwords(&self, username: &str) -> Option<String> {
        self.users.get(username).map(|user| {
            info!(username, "generated password file");
            user.passwords()
        })
    }

    /// Record one password check. The first correct guess pushes exactly one
    /// [`Completion`] onto the leaderboard.
    ///
    /// # Panics
    /// If the leaderboard mutex is poisoned, which means another thread
    /// already panicked; crashing beats serving a corrupt leaderboard.
    pub fn check(&self, username: &str, password: &str, now: Timestamp) -> CheckOutcome {
        // Resolve the attempt before touching the leaderboard so the user
        // shard lock and the leaderboard mutex are never held together.
        let result = match self.users.get_mut(username) {
            None => return CheckOutcome::NotFound,
            Some(mut user) => user.record_attempt(password, now),
        };
        match result {
            AttemptResult::Incorrect => CheckOutcome::Incorrect,
            AttemptResult::AlreadySolved => CheckOutcome::Correct,
            AttemptResult::JustSolved(completion) => {
                info!(
                    username,
                    attempts = completion.attempts_to_solve,
                    "solved"
                );
                self.leaderboard
                    .lock()
                    .expect("leaderboard mutex poisoned")
                    .push(completion);
                CheckOutcome::Correct
            }
        }
    }

    /// Everyone who has solved the challenge, in order of completion.
    ///
    /// # Panics
    /// If the leaderboard mutex is poisoned, which means another thread
    /// already panicked; crashing beats serving a corrupt leaderboard.
    #[must_use]
    pub fn leaders(&self) -> Vec<Completion> {
        self.leaderboard
            .lock()
            .expect("leaderboard mutex poisoned")
            .clone()
    }
}

/// Enforce that a username is 3-32 ASCII letters or digits.
#[must_use]
pub fn valid_username(username: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[a-zA-Z0-9]{3,32}$").expect("username pattern is valid")
    });
    RE.is_match(username)
}
