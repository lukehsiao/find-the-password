use dashmap::DashMap;
use jiff::{Span, Timestamp};
use rand::{distributions::Alphanumeric, rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use tracing::info;

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
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
    #[serde(skip)]
    pub seed: i64,
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

impl User {
    /// Create a new user with a seed and secret
    #[allow(clippy::cast_sign_loss)]
    pub fn new(username: String) -> User {
        // Generate a seed based on username and current time
        let seed = username
            .as_bytes()
            .iter()
            .map(|x| i64::from(*x))
            .sum::<i64>()
            + Timestamp::now().as_millisecond();
        let rng = StdRng::seed_from_u64(seed as u64);
        let secret = rng
            .sample_iter(&Alphanumeric)
            .take(PASS_LEN)
            .map(char::from)
            .collect();

        User {
            username,
            created_at: Timestamp::now(),
            solved_at: None,
            hits_before_solved: 0,
            seed,
            secret,
        }
    }

    /// Get a user-specific list of passwords.
    ///
    /// # Panics
    /// - If the seed cannot be cast to a `usize`
    #[allow(clippy::cast_sign_loss)]
    pub fn get_passwords(&self) -> String {
        let mut rng = StdRng::seed_from_u64(self.seed as u64);
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
        let offset = usize::try_from(self.seed).unwrap() % (NUM_PASSWORDS - OFFSET) + OFFSET;
        passwords.swap(0, offset);

        #[cfg(feature = "ssr")]
        info!(
            user = self.username,
            offset = offset,
            "Generated {NUM_PASSWORDS} passwords"
        );
        // Hack to end the file with a newline
        passwords.push(String::new());
        passwords.join("\n")
    }

    /// Check if the password is the correct one.
    pub fn check_password(&self, password: &str) -> bool {
        if self.secret == password {
            #[cfg(feature = "ssr")]
            info!(
                user = self.username,
                solved_at = %Timestamp::now(),
                "solved"
            );
            true
        } else {
            false
        }
    }
}
