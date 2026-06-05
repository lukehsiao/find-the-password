use std::hash::{DefaultHasher, Hash, Hasher};

use jiff::{Span, Timestamp};
use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};
use serde::{Deserialize, Serialize};

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
// Keep the secret out of the first 15k lines so a naive top-down scan
// can't win in the first few seconds.
const OFFSET: usize = 15_000;

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

#[cfg(test)]
mod tests {
    use hegel::extras::jiff as jiff_gs;
    use hegel::generators::{self, Generator};
    use jiff::SignedDuration;

    use super::{AttemptResult, NUM_PASSWORDS, OFFSET, PASS_LEN, Timestamp, User};

    fn usernames() -> impl Generator<String> {
        generators::from_regex(r"[a-zA-Z0-9]{3,32}").fullmatch(true)
    }

    // Bound to 2000..2090 so the millisecond seed math and the solve-time
    // offsets below can never overflow; the logic doesn't care about the era.
    fn timestamps() -> impl Generator<Timestamp> {
        let min = Timestamp::from_second(946_684_800).unwrap();
        let max = Timestamp::from_second(3_786_825_600).unwrap();
        jiff_gs::timestamps().min_value(min).max_value(max)
    }

    // A guaranteed-wrong guess: longer than any 32-char secret.
    fn wrong_guess(secret: &str) -> String {
        format!("{secret}-nope")
    }

    #[hegel::test(test_cases = 12)]
    fn passwords_contains_the_secret_exactly_once(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let hits = user
            .passwords()
            .lines()
            .filter(|l| *l == user.secret)
            .count();
        assert_eq!(hits, 1);
    }

    #[hegel::test(test_cases = 12)]
    fn passwords_has_60000_lines_and_a_trailing_newline(tc: hegel::TestCase) {
        let file = User::new(tc.draw(usernames()), tc.draw(timestamps())).passwords();
        assert!(file.ends_with('\n'));
        assert_eq!(file.lines().count(), NUM_PASSWORDS);
    }

    #[hegel::test(test_cases = 12)]
    fn every_password_is_32_alphanumeric_chars(tc: hegel::TestCase) {
        let file = User::new(tc.draw(usernames()), tc.draw(timestamps())).passwords();
        for line in file.lines() {
            assert_eq!(line.len(), PASS_LEN);
            assert!(line.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }

    #[hegel::test(test_cases = 12)]
    fn passwords_are_deterministic(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        assert_eq!(user.passwords(), user.passwords());
    }

    #[hegel::test(test_cases = 12)]
    fn secret_is_never_in_the_first_15000_lines(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let index = user
            .passwords()
            .lines()
            .position(|l| l == user.secret)
            .unwrap();
        assert!((OFFSET..NUM_PASSWORDS).contains(&index));
    }

    #[hegel::test(test_cases = 12)]
    fn check_password_accepts_only_the_secret_line(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let file = user.passwords();
        let lines: Vec<&str> = file.lines().collect();
        let index = tc.draw(generators::integers::<usize>().max_value(lines.len() - 1));
        let line = lines[index];
        assert_eq!(user.check_password(line), line == user.secret);
    }

    #[hegel::test]
    fn user_is_deterministic_given_name_and_time(tc: hegel::TestCase) {
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        let a = User::new(name.clone(), now);
        let b = User::new(name, now);
        assert_eq!(a.seed, b.seed);
        assert_eq!(a.secret, b.secret);
        assert_eq!(a.created_at, b.created_at);
    }

    #[hegel::test]
    fn secret_is_32_alphanumeric_chars(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        assert_eq!(user.secret.len(), PASS_LEN);
        assert!(user.secret.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[hegel::test]
    fn wrong_guesses_count_but_never_solve(tc: hegel::TestCase) {
        let mut user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let now = tc.draw(timestamps());
        let attempts = tc.draw(generators::integers::<u64>().max_value(20));
        let wrong = wrong_guess(&user.secret);
        for _ in 0..attempts {
            assert!(matches!(
                user.record_attempt(&wrong, now),
                AttemptResult::Incorrect
            ));
        }
        assert_eq!(user.hits_before_solved, attempts);
        assert!(user.solved_at.is_none());
    }

    #[hegel::test]
    fn correct_guess_solves_once_and_records_the_completion(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let secret = user.secret.clone();
        let wrong = wrong_guess(&secret);

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            user.record_attempt(&wrong, created);
        }

        let elapsed = tc.draw(
            generators::integers::<i64>()
                .min_value(0)
                .max_value(31_536_000),
        );
        let solved_at = created
            .checked_add(SignedDuration::from_secs(elapsed))
            .unwrap();

        match user.record_attempt(&secret, solved_at) {
            AttemptResult::JustSolved(completion) => {
                assert_eq!(completion.attempts_to_solve, warmup + 1);
                // Span has no PartialEq; compare as fixed-unit durations.
                assert_eq!(
                    completion.time_to_solve.fieldwise(),
                    (solved_at - created).fieldwise()
                );
            }
            other => panic!("expected JustSolved, got {other:?}"),
        }

        // Checks after solving report correctness but never change the record.
        let frozen = user.hits_before_solved;
        assert!(matches!(
            user.record_attempt(&secret, solved_at),
            AttemptResult::AlreadySolved
        ));
        assert!(matches!(
            user.record_attempt(&wrong, solved_at),
            AttemptResult::Incorrect
        ));
        assert_eq!(user.hits_before_solved, frozen);
    }
}
