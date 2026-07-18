use std::sync::{Arc, LazyLock, Mutex};

use dashmap::{DashMap, Entry};
use jiff::Timestamp;
use regex::Regex;
use tracing::info;

use crate::{
    error::AppError,
    user::{Completion, ConfirmResult, RosterEntry, User},
};

/// The outcome of checking one password guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    NotFound,
    Incorrect,
    Correct,
}

/// The outcome of one confirmation submission from the user page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    NotFound,
    Throttled,
    Incorrect,
    Confirmed,
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

    /// Record one check-URL guess. Checking only reports correctness; the
    /// challenge is solved by confirming the password via [`Self::confirm`].
    pub fn check(&self, username: &str, password: &str) -> CheckOutcome {
        match self.users.get_mut(username) {
            None => CheckOutcome::NotFound,
            Some(mut user) => {
                if user.record_check(password) {
                    CheckOutcome::Correct
                } else {
                    CheckOutcome::Incorrect
                }
            }
        }
    }

    /// Record one confirmation submission. The first correct confirmation
    /// solves the challenge and pushes exactly one [`Completion`] onto the
    /// leaderboard; correct confirmations after that stay `Confirmed`.
    ///
    /// # Panics
    /// If the leaderboard mutex is poisoned, which means another thread
    /// already panicked; crashing beats serving a corrupt leaderboard.
    pub fn confirm(&self, username: &str, password: &str, now: Timestamp) -> ConfirmOutcome {
        // Resolve the attempt before touching the leaderboard so the user
        // shard lock and the leaderboard mutex are never held together.
        let result = match self.users.get_mut(username) {
            None => return ConfirmOutcome::NotFound,
            Some(mut user) => user.confirm(password, now),
        };
        match result {
            ConfirmResult::Throttled => ConfirmOutcome::Throttled,
            ConfirmResult::Incorrect => ConfirmOutcome::Incorrect,
            ConfirmResult::AlreadySolved => ConfirmOutcome::Confirmed,
            ConfirmResult::JustSolved(completion) => {
                info!(username, attempts = completion.attempts_to_solve, "solved");
                self.leaderboard
                    .lock()
                    .expect("leaderboard mutex poisoned")
                    .push(completion);
                ConfirmOutcome::Confirmed
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

    /// Every registered user with their attempt count so far, sorted by
    /// attempts descending (username ascending as tiebreak).
    ///
    /// Iteration takes each shard's read lock briefly; the sort happens
    /// after all guards are dropped, so the check path is never stalled
    /// behind it.
    #[must_use]
    pub fn roster(&self) -> Vec<RosterEntry> {
        let mut entries: Vec<RosterEntry> = self
            .users
            .iter()
            .map(|user| RosterEntry {
                username: user.username.clone(),
                attempts: user.hits_before_solved,
                solved: user.solved_at.is_some(),
            })
            .collect();
        entries.sort_unstable_by(|a, b| {
            b.attempts
                .cmp(&a.attempts)
                .then_with(|| a.username.cmp(&b.username))
        });
        entries
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

    use super::{AppError, ChallengeStore, CheckOutcome, ConfirmOutcome, valid_username};

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
        assert_eq!(store.check(&name, "anything"), CheckOutcome::NotFound);
    }

    #[hegel::test]
    fn confirm_on_unknown_user_is_not_found(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        assert_eq!(
            store.confirm(&name, "anything", now),
            ConfirmOutcome::NotFound
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
            assert_eq!(store.check(&name, &wrong), CheckOutcome::Incorrect);
        }
        assert!(store.leaders().is_empty());
    }

    // The point of the confirm flow: looping past the correct password reads
    // true but records nothing until the player confirms it.
    #[hegel::test]
    fn correct_checks_never_solve_or_touch_the_leaderboard(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.add_user(&name, now).unwrap();
        let secret = store.get_user(&name).unwrap().secret;

        let hits = tc.draw(generators::integers::<u64>().min_value(1).max_value(10));
        for _ in 0..hits {
            assert_eq!(store.check(&name, &secret), CheckOutcome::Correct);
        }

        assert!(store.leaders().is_empty());
        assert!(store.get_user(&name).unwrap().solved_at.is_none());
        let roster = store.roster();
        let entry = roster.iter().find(|e| e.username == name).unwrap();
        assert!(!entry.solved);
        assert_eq!(entry.attempts, hits);
    }

    #[hegel::test]
    fn first_correct_confirm_records_exactly_one_completion(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.add_user(&name, now).unwrap();
        let secret = store.get_user(&name).unwrap().secret;
        let wrong = format!("{secret}-nope");

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            store.check(&name, &wrong);
        }

        assert_eq!(
            store.confirm(&name, &secret, now),
            ConfirmOutcome::Confirmed
        );
        // Extra guesses after solving must not grow or change the
        // leaderboard; a repeated correct confirmation stays Confirmed.
        assert_eq!(
            store.confirm(&name, &secret, now),
            ConfirmOutcome::Confirmed
        );
        store.check(&name, &secret);
        store.check(&name, &wrong);

        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1);
        assert_eq!(leaders[0].username, name);
        assert_eq!(leaders[0].attempts_to_solve, warmup + 1);
    }

    // The cooldown itself is covered by the User tests; this locks the
    // store-level mapping of a throttled submission.
    #[hegel::test]
    fn confirms_inside_the_cooldown_are_throttled(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.add_user(&name, now).unwrap();

        assert_eq!(
            store.confirm(&name, "wrong", now),
            ConfirmOutcome::Incorrect
        );
        assert_eq!(
            store.confirm(&name, "wrong", now),
            ConfirmOutcome::Throttled
        );
        let roster = store.roster();
        let entry = roster.iter().find(|e| e.username == name).unwrap();
        assert_eq!(entry.attempts, 1, "throttled submissions never count");
    }

    #[hegel::test]
    fn roster_lists_every_registered_user(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let now = tc.draw(timestamps());
        let count = tc.draw(generators::integers::<u64>().max_value(20));
        let names: Vec<String> = (0..count).map(|i| format!("player{i}")).collect();
        for name in &names {
            store.add_user(name, now).unwrap();
        }

        let mut listed: Vec<String> = store.roster().into_iter().map(|e| e.username).collect();
        listed.sort();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(listed, expected);
    }

    #[hegel::test]
    fn roster_is_sorted_by_attempts_desc_then_username(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let now = tc.draw(timestamps());
        let count = tc.draw(generators::integers::<u64>().min_value(2).max_value(8));
        for i in 0..count {
            let name = format!("player{i}");
            store.add_user(&name, now).unwrap();
            let guesses = tc.draw(generators::integers::<u64>().max_value(10));
            for _ in 0..guesses {
                store.check(&name, "wrong");
            }
        }

        let roster = store.roster();
        assert_eq!(roster.len(), usize::try_from(count).unwrap());
        for pair in roster.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                a.attempts > b.attempts || (a.attempts == b.attempts && a.username < b.username),
                "roster must sort by attempts desc, then username asc"
            );
        }
    }

    #[hegel::test]
    fn roster_counts_match_attempts_and_freeze_after_solve(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let now = tc.draw(timestamps());
        store.add_user("solver", now).unwrap();
        store.add_user("grinder", now).unwrap();
        let secret = store.get_user("solver").unwrap().secret;
        let wrong = format!("{secret}-nope");

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            store.check("solver", &wrong);
        }
        // Finding the password counts an attempt but does not solve; the
        // confirmation counts one more and does.
        assert_eq!(store.check("solver", &secret), CheckOutcome::Correct);
        assert!(store.leaders().is_empty());
        assert_eq!(
            store.confirm("solver", &secret, now),
            ConfirmOutcome::Confirmed
        );

        let grinds = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..grinds {
            store.check("grinder", "wrong");
        }

        // Guesses after solving must not move the solver's frozen count.
        store.check("solver", &secret);
        store.check("solver", &wrong);
        store.confirm("solver", &secret, now);

        let roster = store.roster();
        let solver = roster.iter().find(|e| e.username == "solver").unwrap();
        let grinder = roster.iter().find(|e| e.username == "grinder").unwrap();
        assert!(solver.solved);
        assert_eq!(solver.attempts, warmup + 2);
        assert_eq!(solver.attempts, store.leaders()[0].attempts_to_solve);
        assert!(!grinder.solved);
        assert_eq!(grinder.attempts, grinds);
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
                    // The full player flow: miss, hit, then confirm.
                    store.check(name, "wrong");
                    assert_eq!(store.check(name, secret), CheckOutcome::Correct);
                    assert_eq!(store.confirm(name, secret, now), ConfirmOutcome::Confirmed);
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
                        assert_eq!(store_ref.check("hammer", "wrong"), CheckOutcome::Incorrect);
                    }
                });
            }
        });

        assert_eq!(
            store.confirm("hammer", &secret, now),
            ConfirmOutcome::Confirmed
        );
        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1);
        assert_eq!(
            leaders[0].attempts_to_solve,
            threads * guesses_per_thread + 1,
            "every wrong guess must be counted exactly once"
        );
    }

    // Racing correct confirmations must produce exactly one Completion:
    // the winner solves, everyone after lands on the AlreadySolved path,
    // and all of them read Confirmed.
    #[test]
    fn racing_correct_confirms_record_one_completion() {
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
                        store_ref.confirm("racer", secret_ref, now),
                        ConfirmOutcome::Confirmed
                    );
                });
            }
        });

        let leaders = store.leaders();
        assert_eq!(leaders.len(), 1, "racing winners record one completion");
        assert_eq!(leaders[0].attempts_to_solve, 1);
        assert_eq!(
            store.get_user("racer").unwrap().hits_before_solved,
            1,
            "only the winning confirmation counts"
        );
    }

    // Racing wrong confirmations at one instant: the first through the gate
    // is evaluated, the rest are throttled, and only the evaluated one counts.
    #[test]
    fn racing_wrong_confirms_evaluate_exactly_once() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.add_user("racer", now).unwrap();
        let racers: u64 = 16;

        let store_ref = &store;
        let outcomes = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..racers)
                .map(|_| scope.spawn(move || store_ref.confirm("racer", "wrong", now)))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .collect::<Vec<_>>()
        });

        let incorrect = outcomes
            .iter()
            .filter(|&&o| o == ConfirmOutcome::Incorrect)
            .count();
        let throttled = outcomes
            .iter()
            .filter(|&&o| o == ConfirmOutcome::Throttled)
            .count();
        assert_eq!(incorrect, 1, "exactly one submission is evaluated");
        assert_eq!(throttled, usize::try_from(racers).unwrap() - 1);
        assert!(store.leaders().is_empty());
        assert_eq!(store.get_user("racer").unwrap().hits_before_solved, 1);
    }

    // Roster snapshots race against check's shard write locks; this is the
    // homepage-under-solver-load interleaving the feature must tolerate.
    #[test]
    fn roster_snapshots_are_monotone_under_check_contention() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.add_user("hammer", now).unwrap();
        let threads: u64 = 8;
        let guesses_per_thread: u64 = 200;

        let store_ref = &store;
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(move || {
                    for _ in 0..guesses_per_thread {
                        store_ref.check("hammer", "wrong");
                    }
                });
            }

            let mut last = 0;
            for _ in 0..50 {
                let roster = store_ref.roster();
                let entry = roster.iter().find(|e| e.username == "hammer").unwrap();
                assert!(!entry.solved);
                assert!(
                    entry.attempts >= last,
                    "attempt counts must never go backwards"
                );
                last = entry.attempts;
            }
        });

        let roster = store.roster();
        assert_eq!(roster.len(), 1);
        assert_eq!(
            roster[0].attempts,
            threads * guesses_per_thread,
            "every wrong guess must appear in the final roster"
        );
    }
}
