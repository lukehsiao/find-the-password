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
        // Clone the user out so the shard guard drops before the expensive
        // generation; holding it would stall every check on this shard for
        // the several milliseconds the file takes to build.
        let user = self.get_user(username)?;
        info!(username, "generated password file");
        Some(user.passwords())
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
                info!(username, attempts = completion.attempts_to_solve, "solved");
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
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9]{3,32}$").expect("username pattern is valid"));
    RE.is_match(username)
}

#[cfg(test)]
mod tests {
    use hegel::extras::jiff as jiff_gs;
    use hegel::generators::{self, Generator};

    use jiff::Timestamp;

    use super::{AppError, ChallengeStore, CheckOutcome, valid_username};

    fn usernames() -> impl Generator<String> {
        generators::from_regex(r"[a-zA-Z0-9]{3,32}").fullmatch(true)
    }

    fn timestamps() -> impl Generator<Timestamp> {
        let min = Timestamp::from_second(946_684_800).unwrap();
        let max = Timestamp::from_second(3_786_825_600).unwrap();
        jiff_gs::timestamps().min_value(min).max_value(max)
    }

    #[hegel::test]
    fn valid_username_accepts_well_formed_names(tc: hegel::TestCase) {
        assert!(valid_username(&tc.draw(usernames())));
    }

    #[hegel::test]
    fn valid_username_rejects_too_short(tc: hegel::TestCase) {
        let name = tc.draw(generators::from_regex(r"[a-zA-Z0-9]{0,2}").fullmatch(true));
        assert!(!valid_username(&name));
    }

    #[hegel::test]
    fn valid_username_rejects_too_long(tc: hegel::TestCase) {
        let name = tc.draw(generators::from_regex(r"[a-zA-Z0-9]{33,64}").fullmatch(true));
        assert!(!valid_username(&name));
    }

    #[hegel::test]
    fn valid_username_rejects_non_alphanumeric(tc: hegel::TestCase) {
        // A run of valid chars with one disallowed character spliced in.
        let name = tc.draw(
            generators::from_regex(r"[a-zA-Z0-9]{1,15}[^a-zA-Z0-9][a-zA-Z0-9]{1,15}")
                .fullmatch(true),
        );
        assert!(!valid_username(&name));
    }

    #[hegel::test]
    fn add_user_rejects_duplicates(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        assert_eq!(store.add_user(&name, now), Ok(()));
        assert_eq!(store.add_user(&name, now), Err(AppError::UsernameTaken));
    }

    #[hegel::test]
    fn add_user_rejects_invalid_names(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(generators::from_regex(r"[a-zA-Z0-9]{0,2}").fullmatch(true));
        assert_eq!(
            store.add_user(&name, tc.draw(timestamps())),
            Err(AppError::InvalidUsername)
        );
    }

    #[hegel::test]
    fn check_on_unknown_user_is_not_found(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        assert_eq!(
            store.check(&name, "anything", tc.draw(timestamps())),
            CheckOutcome::NotFound
        );
    }

    #[hegel::test]
    fn wrong_guesses_are_incorrect_and_leave_the_board_empty(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.add_user(&name, now).unwrap();

        let wrong = format!("{}-nope", store.get_user(&name).unwrap().secret);
        let attempts = tc.draw(generators::integers::<u64>().max_value(20));
        for _ in 0..attempts {
            assert_eq!(store.check(&name, &wrong, now), CheckOutcome::Incorrect);
        }
        assert!(store.leaders().is_empty());
    }

    #[hegel::test]
    fn first_correct_guess_records_exactly_one_completion(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.add_user(&name, now).unwrap();
        let secret = store.get_user(&name).unwrap().secret;
        let wrong = format!("{secret}-nope");

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            store.check(&name, &wrong, now);
        }

        assert_eq!(store.check(&name, &secret, now), CheckOutcome::Correct);
        // Extra checks after solving must not grow or change the leaderboard.
        store.check(&name, &secret, now);
        store.check(&name, &wrong, now);

        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].username, name);
        assert_eq!(leaders[0].attempts_to_solve, warmup + 1);
    }

    // The single-threaded property tests above can't catch races. These drive
    // the store concurrently to exercise the entry-API guard in add_user and
    // the lock ordering in check that the production path relies on.

    #[test]
    fn racing_add_user_on_one_name_succeeds_exactly_once() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        let racers = 64;

        let wins = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..racers)
                .map(|_| scope.spawn(|| store.add_user("racer", now).is_ok()))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|&won| won)
                .count()
        });

        assert_eq!(wins, 1, "exactly one concurrent registration should win");
        assert!(store.get_user("racer").is_some());
    }

    #[test]
    fn concurrent_solves_record_one_completion_per_user() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        let users = 64;

        let names: Vec<String> = (0..users).map(|i| format!("solver{i}")).collect();
        for name in &names {
            store.add_user(name, now).unwrap();
        }
        let secrets: Vec<String> = names
            .iter()
            .map(|name| store.get_user(name).unwrap().secret)
            .collect();

        let store = &store;
        std::thread::scope(|scope| {
            for (name, secret) in names.iter().zip(&secrets) {
                scope.spawn(move || {
                    // A miss before the hit, so check holds both locks in turn.
                    store.check(name, "wrong", now);
                    assert_eq!(store.check(name, secret, now), CheckOutcome::Correct);
                });
            }
        });

        let mut solved: Vec<String> = store.leaders().into_iter().map(|c| c.username).collect();
        solved.sort();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(solved, expected, "every solver recorded exactly once");
    }

    // Many threads hammering one user is the actual production workload (a
    // kid's parallel solver). No wrong guess may ever be lost.
    #[test]
    fn same_user_contention_counts_every_wrong_guess() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.add_user("hammer", now).unwrap();
        let secret = store.get_user("hammer").unwrap().secret;
        let threads: u64 = 8;
        let guesses_per_thread: u64 = 200;

        let store_ref = &store;
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(move || {
                    for _ in 0..guesses_per_thread {
                        assert_eq!(
                            store_ref.check("hammer", "wrong", now),
                            CheckOutcome::Incorrect
                        );
                    }
                });
            }
        });

        assert_eq!(store.check("hammer", &secret, now), CheckOutcome::Correct);
        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1);
        assert_eq!(
            leaders[0].attempts_to_solve,
            threads * guesses_per_thread + 1,
            "every wrong guess must be counted exactly once"
        );
    }

    // Racing correct guesses must produce exactly one Completion, and the
    // stored count must freeze at the winning attempt.
    #[test]
    fn racing_correct_guesses_record_one_completion() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.add_user("racer", now).unwrap();
        let secret = store.get_user("racer").unwrap().secret;
        let racers: u64 = 16;

        let store_ref = &store;
        let secret_ref = &secret;
        std::thread::scope(|scope| {
            for _ in 0..racers {
                scope.spawn(move || {
                    assert_eq!(
                        store_ref.check("racer", secret_ref, now),
                        CheckOutcome::Correct
                    );
                });
            }
        });

        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1, "racing winners record one completion");
        let attempts = leaders[0].attempts_to_solve;
        assert!((1..=racers).contains(&attempts));
        assert_eq!(
            store.get_user("racer").unwrap().hits_before_solved,
            attempts,
            "the counter freezes at the winning attempt"
        );
    }
}
