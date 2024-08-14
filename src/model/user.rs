use dashmap::DashMap;
use jiff::Timestamp;
use rand::{distributions::Alphanumeric, rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use tracing::info;

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
const OFFSET: usize = 15_000;

/// Collection of all users
pub type UserMap = DashMap<String, User>;

/// Defines all of the state we keep for a particular user.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub username: String,
    pub created_at: Timestamp,
    pub solved_at: Option<Timestamp>,
    pub hits_before_solved: u64,
    pub total_hits: u64,
    #[serde(skip)]
    pub seed: i64,
    #[serde(skip)]
    pub secret: String,
}

impl User {
    /// Create a new user with a seed and secret
    fn new(username: String) -> User {
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
            total_hits: 0,
            seed,
            secret,
        }
    }

    /// Get a user-specific list of passwords.
    fn get_passwords(&self) -> String {
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
        let offset = self.seed as usize % (NUM_PASSWORDS - OFFSET) + OFFSET;
        passwords.swap(0, offset);

        #[cfg(feature = "ssr")]
        info!(
            user = self.username,
            offset = offset,
            "Generated {NUM_PASSWORDS} passwords"
        );

        passwords.join("\n")
    }
}
