use std::hash::{DefaultHasher, Hash, Hasher};

use dashmap::DashMap;
use jiff::{Span, Timestamp};
use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};
use serde::{Deserialize, Serialize};

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
// Keep the secret out of the first 15k lines so a naive top-down scan
// can't win in the first few seconds.
const OFFSET: usize = 15_000;

/// Collection of all users
pub type Users = DashMap<String, User>;

/// Defines all of the state we keep for a particular user.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub username: String,
    pub created_at: Timestamp,
    pub solved_at: Option<Timestamp>,
    pub hits_before_solved: u64,
    // Never serialized: `get_user` sends User to the client, and the seed
    // and secret must not leak to the browser.
    #[serde(skip)]
    pub seed: u64,
    #[serde(skip)]
    pub secret: String,
}

/// Represents an entry in the leaderboard
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Completion {
    pub username: String,
    pub time_to_solve: Span,
    pub attempts_to_solve: u64,
}

/// What a single password check did to a user's state.
#[derive(Debug, Clone)]
pub enum AttemptResult {
    Incorrect,
    AlreadySolved,
    JustSolved(Completion),
}

impl User {
    /// Create a new user with a seed and secret.
    ///
    /// Deterministic given `(username, now)`: the seed mixes a hash of the
    /// username with the creation time, so the password file is reproducible
    /// from stored state while still being unpredictable from the username
    /// alone.
    #[must_use]
    pub fn new(username: String, now: Timestamp) -> User {
        let mut hasher = DefaultHasher::new();
        username.hash(&mut hasher);
        let seed = hasher.finish() ^ now.as_millisecond().cast_unsigned();
        let secret = StdRng::seed_from_u64(seed)
            .sample_iter(&Alphanumeric)
            .take(PASS_LEN)
            .map(char::from)
            .collect();

        User {
            username,
            created_at: now,
            solved_at: None,
            hits_before_solved: 0,
            seed,
            secret,
        }
    }

    /// Get this user's full password file, newline-terminated.
    #[must_use]
    pub fn passwords(&self) -> String {
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut passwords: Vec<String> = (0..NUM_PASSWORDS)
            .map(|_| {
                (&mut rng)
                    .sample_iter(&Alphanumeric)
                    .take(PASS_LEN)
                    .map(char::from)
                    .collect()
            })
            .collect();

        // The first password generated is the secret, so swap it deeper into
        // the list to a seed-derived position.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the modulo result is below NUM_PASSWORDS - OFFSET"
        )]
        let offset = (self.seed % (NUM_PASSWORDS - OFFSET) as u64) as usize + OFFSET;
        passwords.swap(0, offset);

        // join() adds no trailing separator, so an empty final element gives
        // the file its trailing newline.
        passwords.push(String::new());
        passwords.join("\n")
    }

    /// Check if the password is the correct one.
    #[must_use]
    pub fn check_password(&self, password: &str) -> bool {
        self.secret == password
    }

    /// Apply one password check to this user's state.
    ///
    /// Attempts only count while the challenge is unsolved, and the first
    /// correct check produces the one and only [`Completion`]. Checks after
    /// solving still report correctness but change nothing.
    pub fn record_attempt(&mut self, password: &str, now: Timestamp) -> AttemptResult {
        if self.solved_at.is_some() {
            return if self.check_password(password) {
                AttemptResult::AlreadySolved
            } else {
                AttemptResult::Incorrect
            };
        }

        self.hits_before_solved += 1;
        if self.check_password(password) {
            self.solved_at = Some(now);
            AttemptResult::JustSolved(Completion {
                username: self.username.clone(),
                time_to_solve: now - self.created_at,
                attempts_to_solve: self.hits_before_solved,
            })
        } else {
            AttemptResult::Incorrect
        }
    }
}
