use std::sync::Arc;

use dashmap::{DashMap, Entry};
use jiff::Timestamp;
use tracing::info;

use crate::{
    error::AppError,
    player::{Player, Standing},
};

/// The outcome of checking one password guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    NotFound,
    Incorrect,
    Correct,
}

/// The leaderboard and the roster, read together in one pass.
#[derive(Debug, Clone)]
pub struct Standings {
    /// Players who have solved, in solve order (username as tiebreak).
    pub leaderboard: Vec<Standing>,
    /// Every player, by attempts descending (username as tiebreak).
    pub roster: Vec<Standing>,
}

/// In-memory store of all challenge state.
///
/// This is the one interface over the players; the route and page handlers
/// stay thin adapters over it. Cloning is cheap (one `Arc`), and
/// constructing one is the entire test setup. Timestamps are passed in by
/// callers so the store itself never touches the clock.
#[derive(Debug, Clone, Default)]
pub struct ChallengeStore {
    players: Arc<DashMap<String, Player>>,
}

impl ChallengeStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate `username` and join the challenge under it.
    ///
    /// # Errors
    /// - [`AppError::InvalidUsername`] if the name fails [`valid_username`].
    /// - [`AppError::UsernameTaken`] if the name already exists.
    pub fn join(&self, username: &str, now: Timestamp) -> Result<(), AppError> {
        if !valid_username(username) {
            return Err(AppError::InvalidUsername);
        }
        // The entry API reserves the slot atomically, so two concurrent
        // requests for the same name cannot both succeed.
        match self.players.entry(username.to_owned()) {
            Entry::Occupied(_) => Err(AppError::UsernameTaken),
            Entry::Vacant(slot) => {
                slot.insert(Player::new(username.to_owned(), now));
                info!(username, "joined");
                Ok(())
            }
        }
    }

    /// A player's standing, if they have joined.
    #[must_use]
    pub fn standing(&self, username: &str) -> Option<Standing> {
        self.players.get(username).map(|player| player.standing())
    }

    /// Render a player's password file, if they have joined.
    #[must_use]
    pub fn password_file(&self, username: &str) -> Option<String> {
        // Copy the file out so the shard guard drops before the expensive
        // render; holding it would stall every check on this shard for the
        // several milliseconds the file takes to build.
        let file = self.players.get(username)?.password_file();
        info!(username, "rendered password file");
        Some(file.render())
    }

    /// Record one check. A check only reports correctness; the challenge is
    /// solved by confirming the password via [`Self::confirm`].
    pub fn check(&self, username: &str, password: &str) -> CheckOutcome {
        match self.players.get_mut(username) {
            None => CheckOutcome::NotFound,
            Some(mut player) => {
                if player.check(password) {
                    CheckOutcome::Correct
                } else {
                    CheckOutcome::Incorrect
                }
            }
        }
    }

    /// Record one confirmation submission, or `None` for an unknown player.
    /// The first correct confirmation solves the challenge; correct
    /// confirmations after that are still `Ok`.
    ///
    /// # Errors
    /// The player's mistake, as [`Player::confirm`] reports it.
    pub fn confirm(
        &self,
        username: &str,
        password: &str,
        now: Timestamp,
    ) -> Option<Result<(), AppError>> {
        // Log after the shard guard drops, so checks on this shard never
        // wait on log output.
        let (result, solve) = {
            let mut player = self.players.get_mut(username)?;
            let result = player.confirm(password, now);
            let solve = (result == Ok(true)).then(|| player.standing());
            (result, solve)
        };
        if let Some(solve) = solve {
            info!(username, attempts = solve.attempts, "solved");
        }
        Some(result.map(|_| ()))
    }

    /// The leaderboard and the roster from one walk over the players, so
    /// the two always agree.
    ///
    /// Iteration takes each shard's read lock briefly; the sorts happen
    /// after all guards are dropped, so the check path is never stalled
    /// behind them.
    #[must_use]
    pub fn standings(&self) -> Standings {
        let mut solves = Vec::new();
        let mut roster = Vec::with_capacity(self.players.len());
        for player in self.players.iter() {
            let standing = player.standing();
            if let Some(solved_at) = player.solved_at() {
                solves.push((solved_at, standing.clone()));
            }
            roster.push(standing);
        }
        solves.sort_unstable_by(|(a_at, a), (b_at, b)| {
            a_at.cmp(b_at).then_with(|| a.username.cmp(&b.username))
        });
        roster.sort_unstable_by(|a, b| {
            b.attempts
                .cmp(&a.attempts)
                .then_with(|| a.username.cmp(&b.username))
        });
        Standings {
            leaderboard: solves.into_iter().map(|(_, standing)| standing).collect(),
            roster,
        }
    }

    /// The secret of a player, the oracle tests use for the right answer.
    #[cfg(test)]
    pub(crate) fn secret(&self, username: &str) -> Option<String> {
        Some(self.players.get(username)?.secret().to_owned())
    }
}

/// Whether `username` is 3-32 ASCII letters, digits, hyphens, periods, or
/// underscores.
///
/// Usernames are spliced verbatim into URL paths (`/u/{username}/...`), so
/// every allowed character must be URL-unreserved (RFC 3986). The 3-character
/// minimum also keeps out `.` and `..`, the only names a client would
/// collapse as path dot-segments before the request ever reaches the router.
fn valid_username(username: &str) -> bool {
    (3..=32).contains(&username.len())
        && username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_'))
}

#[cfg(test)]
mod tests {
    use hegel::generators;
    use jiff::{SignedDuration, Timestamp};

    use super::{AppError, ChallengeStore, CheckOutcome};
    use crate::testing::{timestamps, usernames, wrong_guess};

    #[hegel::test]
    fn join_accepts_well_formed_names(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        assert_eq!(store.join(&name, tc.draw(timestamps())), Ok(()));
    }

    #[hegel::test]
    fn join_rejects_names_shorter_than_3_or_longer_than_32(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let pattern = if tc.draw(generators::booleans()) {
            r"[a-zA-Z0-9._-]{0,2}"
        } else {
            r"[a-zA-Z0-9._-]{33,64}"
        };
        let name = tc.draw(generators::from_regex(pattern).fullmatch(true));
        assert_eq!(
            store.join(&name, tc.draw(timestamps())),
            Err(AppError::InvalidUsername)
        );
    }

    // One character outside the allowed set, anywhere in an otherwise valid
    // name, is enough to reject it. Half the draws take the character from
    // printable ASCII, where URL-reserved characters like `/` and `?` live
    // and a broad class alone would rarely land.
    #[hegel::test]
    fn join_rejects_any_other_character(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let other = if tc.draw(generators::booleans()) {
            r"[ -,/:-@\[-\^`{-~]"
        } else {
            r"[^a-zA-Z0-9._-]"
        };
        let part = || generators::from_regex(r"[a-zA-Z0-9._-]{1,15}").fullmatch(true);
        let name = format!(
            "{}{}{}",
            tc.draw(part()),
            tc.draw(generators::from_regex(other).fullmatch(true)),
            tc.draw(part())
        );
        assert_eq!(
            store.join(&name, tc.draw(timestamps())),
            Err(AppError::InvalidUsername)
        );
    }

    // Usernames become URL path segments, so the names a client would
    // rewrite as dot-segments must stay invalid. The length rule alone
    // guarantees this; `...` and friends are ordinary segments and are fine.
    #[test]
    fn dot_segment_names_stay_invalid() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        assert_eq!(store.join(".", now), Err(AppError::InvalidUsername));
        assert_eq!(store.join("..", now), Err(AppError::InvalidUsername));
        assert_eq!(store.join("...", now), Ok(()));
        assert_eq!(store.join("a.b-c_d", now), Ok(()));
    }

    #[hegel::test]
    fn join_rejects_duplicates(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        assert_eq!(store.join(&name, now), Ok(()));
        assert_eq!(store.join(&name, now), Err(AppError::UsernameTaken));
    }

    #[hegel::test]
    fn check_on_unknown_player_is_not_found(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        assert_eq!(store.check(&name, "anything"), CheckOutcome::NotFound);
    }

    #[hegel::test]
    fn confirm_on_unknown_player_is_not_found(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        assert_eq!(store.confirm(&name, "anything", now), None);
    }

    #[hegel::test]
    fn wrong_guesses_are_incorrect_and_leave_the_board_empty(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.join(&name, now).unwrap();

        let wrong = wrong_guess(&store.secret(&name).unwrap());
        let attempts = tc.draw(generators::integers::<u64>().max_value(20));
        for _ in 0..attempts {
            assert_eq!(store.check(&name, &wrong), CheckOutcome::Incorrect);
        }
        assert!(store.standings().leaderboard.is_empty());
    }

    // The point of the confirm flow: looping past the correct password reads
    // true but records nothing until the player confirms it.
    #[hegel::test]
    fn correct_checks_never_solve_or_touch_the_leaderboard(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.join(&name, now).unwrap();
        let secret = store.secret(&name).unwrap();

        let hits = tc.draw(generators::integers::<u64>().min_value(1).max_value(10));
        for _ in 0..hits {
            assert_eq!(store.check(&name, &secret), CheckOutcome::Correct);
        }

        assert!(store.standings().leaderboard.is_empty());
        let standing = store.standing(&name).unwrap();
        assert!(standing.time_to_solve.is_none());
        assert_eq!(standing.attempts, hits);
        let roster = store.standings().roster;
        let entry = roster.iter().find(|e| e.username == name).unwrap();
        assert!(entry.time_to_solve.is_none());
        assert_eq!(entry.attempts, hits);
    }

    #[hegel::test]
    fn first_correct_confirm_puts_one_solve_on_the_leaderboard(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let joined = tc.draw(timestamps());
        store.join(&name, joined).unwrap();
        let secret = store.secret(&name).unwrap();
        let wrong = wrong_guess(&secret);

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            store.check(&name, &wrong);
        }

        let elapsed = tc.draw(
            generators::integers::<i64>()
                .min_value(0)
                .max_value(86_400 * 30),
        );
        let solved = joined + SignedDuration::from_secs(elapsed);
        assert_eq!(store.confirm(&name, &secret, solved), Some(Ok(())));
        // Extra guesses after solving must not grow or change the
        // leaderboard; a repeated correct confirmation is still Ok.
        assert_eq!(store.confirm(&name, &secret, solved), Some(Ok(())));
        store.check(&name, &secret);
        store.check(&name, &wrong);

        let leaderboard = store.standings().leaderboard;
        assert_eq!(leaderboard.len(), 1);
        assert_eq!(leaderboard[0].username, name);
        assert_eq!(leaderboard[0].attempts, warmup + 1);
        // Span has no PartialEq; compare as fixed-unit durations.
        assert_eq!(
            leaderboard[0].time_to_solve.unwrap().fieldwise(),
            (solved - joined).fieldwise()
        );
    }

    // The leaderboard is in solve order whatever order players joined in;
    // simultaneous solves fall back to username order.
    #[hegel::test]
    fn leaderboard_is_in_solve_order_then_username(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let start = tc.draw(timestamps());
        let count = tc.draw(generators::integers::<u64>().min_value(2).max_value(8));
        for i in 0..count {
            let name = format!("player{i}");
            store.join(&name, start).unwrap();
            let secret = store.secret(&name).unwrap();
            // Few distinct offsets, so ties are common.
            let offset = tc.draw(generators::integers::<i64>().min_value(0).max_value(3));
            store.confirm(&name, &secret, start + SignedDuration::from_secs(offset));
        }

        let leaderboard = store.standings().leaderboard;
        assert_eq!(leaderboard.len(), usize::try_from(count).unwrap());
        for pair in leaderboard.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let order = a
                .time_to_solve
                .unwrap()
                .compare(b.time_to_solve.unwrap())
                .unwrap()
                .then_with(|| a.username.cmp(&b.username));
            assert!(order.is_lt(), "leaderboard must sort by solve, then name");
        }
    }

    // The cooldown itself is covered by the Player tests; this locks the
    // store-level mapping of a throttled submission.
    #[hegel::test]
    fn confirms_inside_the_cooldown_are_throttled(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        store.join(&name, now).unwrap();

        assert_eq!(
            store.confirm(&name, "wrong", now),
            Some(Err(AppError::WrongPassword))
        );
        assert!(matches!(
            store.confirm(&name, "wrong", now),
            Some(Err(AppError::ConfirmThrottled { .. }))
        ));
        let roster = store.standings().roster;
        let entry = roster.iter().find(|e| e.username == name).unwrap();
        assert_eq!(entry.attempts, 1, "throttled submissions never count");
    }

    #[hegel::test]
    fn roster_lists_every_player(tc: hegel::TestCase) {
        let store = ChallengeStore::new();
        let now = tc.draw(timestamps());
        let count = tc.draw(generators::integers::<u64>().max_value(20));
        let names: Vec<String> = (0..count).map(|i| format!("player{i}")).collect();
        for name in &names {
            store.join(name, now).unwrap();
        }

        let mut listed: Vec<String> = store
            .standings()
            .roster
            .into_iter()
            .map(|e| e.username)
            .collect();
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
            store.join(&name, now).unwrap();
            let guesses = tc.draw(generators::integers::<u64>().max_value(10));
            for _ in 0..guesses {
                store.check(&name, "wrong");
            }
        }

        let roster = store.standings().roster;
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
        store.join("solver", now).unwrap();
        store.join("grinder", now).unwrap();
        let secret = store.secret("solver").unwrap();
        let wrong = wrong_guess(&secret);

        let warmup = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..warmup {
            store.check("solver", &wrong);
        }
        // Finding the password counts an attempt but does not solve; the
        // confirmation counts one more and does.
        assert_eq!(store.check("solver", &secret), CheckOutcome::Correct);
        assert!(store.standings().leaderboard.is_empty());
        assert_eq!(store.confirm("solver", &secret, now), Some(Ok(())));

        let grinds = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..grinds {
            store.check("grinder", "wrong");
        }

        // Guesses after solving must not move the solver's frozen count.
        store.check("solver", &secret);
        store.check("solver", &wrong);
        store.confirm("solver", &secret, now);

        let roster = store.standings().roster;
        let solver = roster.iter().find(|e| e.username == "solver").unwrap();
        let grinder = roster.iter().find(|e| e.username == "grinder").unwrap();
        assert!(solver.time_to_solve.is_some());
        assert_eq!(solver.attempts, warmup + 2);
        assert_eq!(solver.attempts, store.standings().leaderboard[0].attempts);
        assert!(grinder.time_to_solve.is_none());
        assert_eq!(grinder.attempts, grinds);
    }

    // The single-threaded property tests above can't catch races. These drive
    // the store concurrently to exercise the entry-API guard in join and
    // the lock ordering in check that the production path relies on.

    #[test]
    fn racing_joins_on_one_name_succeed_exactly_once() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        let racers = 64;

        let wins = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..racers)
                .map(|_| scope.spawn(|| store.join("racer", now).is_ok()))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|&won| won)
                .count()
        });

        assert_eq!(wins, 1, "exactly one concurrent registration should win");
        assert!(store.standing("racer").is_some());
    }

    #[test]
    fn concurrent_solves_put_every_player_on_the_leaderboard_once() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        let players = 64;

        let names: Vec<String> = (0..players).map(|i| format!("solver{i}")).collect();
        for name in &names {
            store.join(name, now).unwrap();
        }
        let secrets: Vec<String> = names
            .iter()
            .map(|name| store.secret(name).unwrap())
            .collect();

        let store = &store;
        std::thread::scope(|scope| {
            for (name, secret) in names.iter().zip(&secrets) {
                scope.spawn(move || {
                    // The full player flow: miss, hit, then confirm.
                    store.check(name, "wrong");
                    assert_eq!(store.check(name, secret), CheckOutcome::Correct);
                    assert_eq!(store.confirm(name, secret, now), Some(Ok(())));
                });
            }
        });

        let mut solved: Vec<String> = store
            .standings()
            .leaderboard
            .into_iter()
            .map(|c| c.username)
            .collect();
        solved.sort();
        let mut expected = names.clone();
        expected.sort();
        assert_eq!(solved, expected, "every solver appears exactly once");
    }

    // Many threads hammering one player is the actual production workload (a
    // kid's parallel solver). No wrong guess may ever be lost.
    #[test]
    fn same_player_contention_counts_every_wrong_guess() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.join("hammer", now).unwrap();
        let secret = store.secret("hammer").unwrap();
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

        assert_eq!(store.confirm("hammer", &secret, now), Some(Ok(())));
        let leaderboard = store.standings().leaderboard;
        assert_eq!(leaderboard.len(), 1);
        assert_eq!(
            leaderboard[0].attempts,
            threads * guesses_per_thread + 1,
            "every wrong guess must be counted exactly once"
        );
    }

    // Racing correct confirmations must solve exactly once: the winner
    // solves, everyone after lands on the already-solved path, and all of
    // them read Ok.
    #[test]
    fn racing_correct_confirms_solve_once() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.join("racer", now).unwrap();
        let secret = store.secret("racer").unwrap();
        let racers: u64 = 16;

        let store_ref = &store;
        let secret_ref = &secret;
        std::thread::scope(|scope| {
            for _ in 0..racers {
                scope.spawn(move || {
                    assert_eq!(store_ref.confirm("racer", secret_ref, now), Some(Ok(())));
                });
            }
        });

        let leaderboard = store.standings().leaderboard;
        assert_eq!(leaderboard.len(), 1, "racing winners solve once");
        assert_eq!(leaderboard[0].attempts, 1);
        assert_eq!(
            store.standing("racer").unwrap().attempts,
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
        store.join("racer", now).unwrap();
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
            .filter(|&&o| o == Some(Err(AppError::WrongPassword)))
            .count();
        let throttled = outcomes
            .iter()
            .filter(|o| matches!(o, Some(Err(AppError::ConfirmThrottled { .. }))))
            .count();
        assert_eq!(incorrect, 1, "exactly one submission is evaluated");
        assert_eq!(throttled, usize::try_from(racers).unwrap() - 1);
        assert!(store.standings().leaderboard.is_empty());
        assert_eq!(store.standing("racer").unwrap().attempts, 1);
    }

    // Roster snapshots race against check's shard write locks; this is the
    // homepage-under-solver-load interleaving the feature must tolerate.
    #[test]
    fn roster_snapshots_are_monotone_under_check_contention() {
        let store = ChallengeStore::new();
        let now = Timestamp::UNIX_EPOCH;
        store.join("hammer", now).unwrap();
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
                let roster = store_ref.standings().roster;
                let entry = roster.iter().find(|e| e.username == "hammer").unwrap();
                assert!(entry.time_to_solve.is_none());
                assert!(
                    entry.attempts >= last,
                    "attempt counts must never go backwards"
                );
                last = entry.attempts;
            }
        });

        let roster = store.standings().roster;
        assert_eq!(roster.len(), 1);
        assert_eq!(
            roster[0].attempts,
            threads * guesses_per_thread,
            "every wrong guess must appear in the final roster"
        );
    }
}
