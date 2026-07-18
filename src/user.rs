use std::hash::{DefaultHasher, Hash, Hasher};

use jiff::{SignedDuration, Span, Timestamp};
use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};
use serde::{Deserialize, Serialize};

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
// Keep the secret out of the first 15k lines so a naive top-down scan
// can't win in the first few seconds.
const OFFSET: usize = 15_000;
// One confirmation evaluated per 10 seconds: a player retyping after a typo
// barely notices the wait, while looping all 60k passwords through the
// confirmation form would take about a week. Deliberately not advertised in
// the instructions; players only meet it through the throttle error. That
// error message (AppError::ConfirmThrottled) repeats the value in prose, as
// does the Retry-After header this constant is crate-visible for; update
// them together with this.
pub(crate) const CONFIRM_COOLDOWN: SignedDuration = SignedDuration::from_secs(10);

/// Defines all of the state we keep for a particular user.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub username: String,
    pub created_at: Timestamp,
    pub solved_at: Option<Timestamp>,
    pub hits_before_solved: u64,
    // Never serialized: `get_user` sends User to the client, and the seed
    // and secret must not leak to the browser. Visibility is restricted to
    // match: seed is module-private, and the secret is readable only within
    // the crate (the store and the in-crate tests).
    #[serde(skip)]
    seed: u64,
    #[serde(skip)]
    pub(crate) secret: String,
    // When the last evaluated confirmation happened; gates the cooldown.
    // Server-side bookkeeping only, so it is never serialized either.
    #[serde(skip)]
    last_confirm_at: Option<Timestamp>,
}

/// Represents an entry in the leaderboard
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Completion {
    pub username: String,
    pub time_to_solve: Span,
    pub attempts_to_solve: u64,
}

/// One row in the homepage roster of all registered players.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub username: String,
    pub attempts: u64,
    pub solved: bool,
}

/// What a single confirmation submission did to a user's state.
#[derive(Debug, Clone)]
pub enum ConfirmResult {
    /// Rejected without evaluating the password: submitted within
    /// [`CONFIRM_COOLDOWN`] of the last evaluated confirmation.
    Throttled,
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
            last_confirm_at: None,
        }
    }

    /// Get this user's full password file, newline-terminated.
    #[must_use]
    #[expect(
        clippy::missing_panics_doc,
        reason = "the buffer is ASCII alphanumerics and newlines by construction"
    )]
    pub fn passwords(&self) -> String {
        // Line i is the 32 bytes at i * (PASS_LEN + 1), followed by its
        // newline.
        let mut rng = StdRng::seed_from_u64(self.seed);
        let mut buf: Vec<u8> = Vec::with_capacity(NUM_PASSWORDS * (PASS_LEN + 1));
        for _ in 0..NUM_PASSWORDS {
            buf.extend((&mut rng).sample_iter(&Alphanumeric).take(PASS_LEN));
            buf.push(b'\n');
        }

        // The first password generated is the secret, so swap it deeper into
        // the list to a seed-derived position.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the modulo result is below NUM_PASSWORDS - OFFSET"
        )]
        let offset = (self.seed % (NUM_PASSWORDS - OFFSET) as u64) as usize + OFFSET;
        let (head, tail) = buf.split_at_mut(offset * (PASS_LEN + 1));
        head[..PASS_LEN].swap_with_slice(&mut tail[..PASS_LEN]);

        String::from_utf8(buf).expect("Alphanumeric and newlines are valid UTF-8")
    }

    /// Check if the password is the correct one.
    #[must_use]
    pub fn check_password(&self, password: &str) -> bool {
        self.secret == password
    }

    /// Record one check-URL guess and report whether it was correct.
    ///
    /// Checking never solves the challenge, no matter how many times the
    /// correct password goes past; solving requires confirming the password
    /// via [`User::confirm`]. Attempts only count while the challenge is
    /// unsolved.
    pub fn record_check(&mut self, password: &str) -> bool {
        if self.solved_at.is_none() {
            self.hits_before_solved += 1;
        }
        self.check_password(password)
    }

    /// Apply one confirmation submission, which is what actually solves the
    /// challenge.
    ///
    /// The first correct confirmation produces the one and only
    /// [`Completion`]; confirmations after solving still report correctness
    /// but change nothing.
    ///
    /// At most one submission per [`CONFIRM_COOLDOWN`] is evaluated, so this
    /// path cannot be brute-forced like the check URL. Throttled submissions
    /// are not evaluated, do not count as attempts, and do not extend the
    /// window.
    pub fn confirm(&mut self, password: &str, now: Timestamp) -> ConfirmResult {
        if self.solved_at.is_some() {
            return if self.check_password(password) {
                ConfirmResult::AlreadySolved
            } else {
                ConfirmResult::Incorrect
            };
        }

        if let Some(last) = self.last_confirm_at
            && now.duration_since(last) < CONFIRM_COOLDOWN
        {
            return ConfirmResult::Throttled;
        }
        self.last_confirm_at = Some(now);

        self.hits_before_solved += 1;
        if self.check_password(password) {
            self.solved_at = Some(now);
            ConfirmResult::JustSolved(Completion {
                username: self.username.clone(),
                time_to_solve: now - self.created_at,
                attempts_to_solve: self.hits_before_solved,
            })
        } else {
            ConfirmResult::Incorrect
        }
    }
}

#[cfg(test)]
mod tests {
    use hegel::extras::jiff as jiff_gs;
    use hegel::generators::{self, Generator};
    use jiff::SignedDuration;
    use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};

    use super::{
        CONFIRM_COOLDOWN, ConfirmResult, NUM_PASSWORDS, OFFSET, PASS_LEN, Timestamp, User,
    };

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

    // Reference oracle: build the file the simple way, one String per line,
    // swapped, then joined. passwords() must match it byte for byte, or a
    // user's file would change under them mid-challenge.
    #[hegel::test(test_cases = 12)]
    fn passwords_matches_the_per_string_reference(tc: hegel::TestCase) {
        let user = User::new(tc.draw(usernames()), tc.draw(timestamps()));

        let mut rng = StdRng::seed_from_u64(user.seed);
        let mut reference: Vec<String> = (0..NUM_PASSWORDS)
            .map(|_| {
                (&mut rng)
                    .sample_iter(&Alphanumeric)
                    .take(PASS_LEN)
                    .map(char::from)
                    .collect()
            })
            .collect();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the modulo result is below NUM_PASSWORDS - OFFSET"
        )]
        let offset = (user.seed % (NUM_PASSWORDS - OFFSET) as u64) as usize + OFFSET;
        reference.swap(0, offset);
        reference.push(String::new());

        assert_eq!(user.passwords(), reference.join("\n"));
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

    // Regression guard: the original byte-sum seed gave anagram usernames the
    // same secret at the same instant. Hashing the username fixed it.
    #[test]
    fn anagram_usernames_get_different_secrets() {
        let now = Timestamp::UNIX_EPOCH;
        let abc = User::new("abc".to_string(), now);
        let bca = User::new("bca".to_string(), now);
        assert_ne!(abc.secret, bca.secret);
    }

    #[hegel::test]
    fn wrong_checks_count_but_never_solve(tc: hegel::TestCase) {
        let mut user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let attempts = tc.draw(generators::integers::<u64>().max_value(20));
        let wrong = wrong_guess(&user.secret);
        for _ in 0..attempts {
            assert!(!user.record_check(&wrong));
        }
        assert_eq!(user.hits_before_solved, attempts);
        assert!(user.solved_at.is_none());
    }

    // The core of the feature: a script can loop right past the correct
    // password and the challenge stays unsolved until it is confirmed.
    #[hegel::test]
    fn correct_checks_report_true_but_never_solve(tc: hegel::TestCase) {
        let mut user = User::new(tc.draw(usernames()), tc.draw(timestamps()));
        let secret = user.secret.clone();
        let hits = tc.draw(generators::integers::<u64>().min_value(1).max_value(20));
        for _ in 0..hits {
            assert!(user.record_check(&secret));
        }
        assert!(user.solved_at.is_none());
        assert_eq!(
            user.hits_before_solved, hits,
            "found-but-unconfirmed checks keep counting"
        );
    }

    #[hegel::test]
    fn correct_confirm_solves_once_and_records_the_completion(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let secret = user.secret.clone();
        let wrong = wrong_guess(&secret);

        let checks = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..checks {
            user.record_check(&wrong);
        }

        // Wrong confirmations spaced past the cooldown; each one counts.
        let confirms = tc.draw(generators::integers::<u64>().max_value(5));
        let mut now = created;
        for _ in 0..confirms {
            let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
            now = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
            assert!(matches!(
                user.confirm(&wrong, now),
                ConfirmResult::Incorrect
            ));
        }

        let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
        let solved_at = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
        match user.confirm(&secret, solved_at) {
            ConfirmResult::JustSolved(completion) => {
                assert_eq!(completion.attempts_to_solve, checks + confirms + 1);
                // Span has no PartialEq; compare as fixed-unit durations.
                assert_eq!(
                    completion.time_to_solve.fieldwise(),
                    (solved_at - created).fieldwise()
                );
            }
            other => panic!("expected JustSolved, got {other:?}"),
        }

        // Checks and confirmations after solving report correctness but
        // never change the record.
        let frozen = user.hits_before_solved;
        assert!(matches!(
            user.confirm(&secret, solved_at),
            ConfirmResult::AlreadySolved
        ));
        assert!(matches!(
            user.confirm(&wrong, solved_at),
            ConfirmResult::Incorrect
        ));
        assert!(user.record_check(&secret));
        assert!(!user.record_check(&wrong));
        assert_eq!(user.hits_before_solved, frozen);
    }

    // The throttle gate sits in front of evaluation: inside the window even
    // the correct password is rejected, and nothing is counted.
    #[hegel::test]
    fn confirms_within_the_cooldown_are_throttled_and_uncounted(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let secret = user.secret.clone();
        let wrong = wrong_guess(&secret);

        assert!(matches!(
            user.confirm(&wrong, created),
            ConfirmResult::Incorrect
        ));

        let within = tc.draw(generators::integers::<i64>().min_value(0).max_value(9));
        let again = created
            .checked_add(SignedDuration::from_secs(within))
            .unwrap();
        assert!(matches!(
            user.confirm(&secret, again),
            ConfirmResult::Throttled
        ));
        assert!(user.solved_at.is_none());
        assert_eq!(user.hits_before_solved, 1);
    }

    // The clock can step backwards (NTP). A confirmation timestamped before
    // the last evaluated one sits inside the window and is throttled, no
    // matter how far back the rewind goes; the gate heals on its own once
    // the clock passes the old mark plus the cooldown.
    #[hegel::test]
    fn confirms_with_a_rewound_clock_are_throttled(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let secret = user.secret.clone();
        let wrong = wrong_guess(&secret);

        assert!(matches!(
            user.confirm(&wrong, created),
            ConfirmResult::Incorrect
        ));

        let rewind = tc.draw(generators::integers::<i64>().min_value(1).max_value(86_400));
        let earlier = created
            .checked_sub(SignedDuration::from_secs(rewind))
            .unwrap();
        assert!(matches!(
            user.confirm(&secret, earlier),
            ConfirmResult::Throttled
        ));
        assert!(user.solved_at.is_none());
        assert_eq!(user.hits_before_solved, 1);
    }

    // Checks and confirmations interleave in the wild. Every evaluated
    // guess counts exactly once regardless of the order, and wrong guesses
    // never solve.
    #[hegel::test]
    fn interleaved_wrong_guesses_each_count_once(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let wrong = wrong_guess(&user.secret);

        let guesses = tc.draw(generators::integers::<u64>().max_value(20));
        let mut now = created;
        for _ in 0..guesses {
            if tc.draw(generators::booleans()) {
                assert!(!user.record_check(&wrong));
            } else {
                let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
                now = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
                assert!(matches!(
                    user.confirm(&wrong, now),
                    ConfirmResult::Incorrect
                ));
            }
        }
        assert_eq!(user.hits_before_solved, guesses);
        assert!(user.solved_at.is_none());
    }

    // Throttled submissions must not extend the window, or a hammering
    // script could lock a player out of confirming forever.
    #[hegel::test]
    fn throttled_confirms_do_not_extend_the_cooldown_window(tc: hegel::TestCase) {
        let created = tc.draw(timestamps());
        let mut user = User::new(tc.draw(usernames()), created);
        let secret = user.secret.clone();
        let wrong = wrong_guess(&secret);

        assert!(matches!(
            user.confirm(&wrong, created),
            ConfirmResult::Incorrect
        ));
        let hammers = tc.draw(generators::integers::<i64>().min_value(1).max_value(9));
        for s in 1..=hammers {
            let at = created.checked_add(SignedDuration::from_secs(s)).unwrap();
            assert!(matches!(user.confirm(&wrong, at), ConfirmResult::Throttled));
        }

        // The window is measured from the last *evaluated* confirmation, so
        // it ends exactly one cooldown after the first submission.
        let at = created.checked_add(CONFIRM_COOLDOWN).unwrap();
        assert!(matches!(
            user.confirm(&secret, at),
            ConfirmResult::JustSolved(_)
        ));
        assert_eq!(user.hits_before_solved, 2);
    }
}
