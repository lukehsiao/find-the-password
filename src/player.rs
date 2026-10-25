use std::hash::{DefaultHasher, Hash, Hasher};

use jiff::{SignedDuration, Span, Timestamp};
use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};

use crate::error::AppError;

const NUM_PASSWORDS: usize = 60_000;
const PASS_LEN: usize = 32;
// Keep the secret out of the first 15k lines so a naive top-down scan
// can't win in the first few seconds.
const OFFSET: usize = 15_000;
// One confirmation evaluated per 10 seconds: a player retyping after a typo
// barely notices the wait, while looping all 60k passwords through the
// confirmation form would take about a week. Deliberately not advertised in
// the instructions; players only meet it through the throttle error, whose
// message states this value.
pub(crate) const CONFIRM_COOLDOWN: SignedDuration = SignedDuration::from_secs(10);

/// Everything the challenge keeps about one player.
///
/// Every field is private to this module, so nothing outside it can read
/// the secret or recompute a solve. Other modules see a player only
/// through a [`Standing`].
#[derive(Debug)]
pub(crate) struct Player {
    username: String,
    joined_at: Timestamp,
    solved_at: Option<Timestamp>,
    attempts: u64,
    seed: u64,
    secret: String,
    // When the last evaluated confirmation happened; gates the cooldown.
    last_confirm_at: Option<Timestamp>,
}

/// One player's place in the standings: their attempts and, once solved,
/// their time to solve.
#[derive(Debug, Clone)]
pub struct Standing {
    pub username: String,
    pub attempts: u64,
    pub time_to_solve: Option<Span>,
}

/// A player's password file, ready to render.
///
/// Copying it out costs nothing, so callers can drop any lock on the player
/// before paying for [`PasswordFile::render`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct PasswordFile {
    seed: u64,
}

impl PasswordFile {
    /// The full file, newline-terminated.
    #[must_use]
    pub(crate) fn render(self) -> String {
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
}

impl Player {
    /// A player who joins as `username` at `now`.
    ///
    /// Deterministic given `(username, now)`: the seed mixes a hash of the
    /// username with the join time, so the password file is reproducible
    /// from stored state while still being unpredictable from the username
    /// alone.
    #[must_use]
    pub(crate) fn new(username: String, now: Timestamp) -> Player {
        let mut hasher = DefaultHasher::new();
        username.hash(&mut hasher);
        let seed = hasher.finish() ^ now.as_millisecond().cast_unsigned();
        let secret = StdRng::seed_from_u64(seed)
            .sample_iter(&Alphanumeric)
            .take(PASS_LEN)
            .map(char::from)
            .collect();

        Player {
            username,
            joined_at: now,
            solved_at: None,
            attempts: 0,
            seed,
            secret,
            last_confirm_at: None,
        }
    }

    #[must_use]
    pub(crate) fn password_file(&self) -> PasswordFile {
        PasswordFile { seed: self.seed }
    }

    /// When this player solved, if they have.
    #[must_use]
    pub(crate) fn solved_at(&self) -> Option<Timestamp> {
        self.solved_at
    }

    #[must_use]
    pub(crate) fn standing(&self) -> Standing {
        Standing {
            username: self.username.clone(),
            attempts: self.attempts,
            time_to_solve: self.solved_at.map(|solved_at| solved_at - self.joined_at),
        }
    }

    #[cfg(test)]
    pub(crate) fn secret(&self) -> &str {
        &self.secret
    }

    /// Record one check and report whether `password` is the secret.
    ///
    /// A check never solves the challenge, no matter how many times the
    /// secret goes past; only [`Player::confirm`] solves. Attempts only
    /// count while the challenge is unsolved.
    pub(crate) fn check(&mut self, password: &str) -> bool {
        if self.solved_at.is_none() {
            self.attempts += 1;
        }
        self.secret == password
    }

    /// Apply one confirmation submission, which is what actually solves the
    /// challenge.
    ///
    /// Returns `Ok(true)` for the solve itself, the first correct
    /// confirmation, and `Ok(false)` for a correct confirmation after it.
    /// Confirmations after solving report correctness but change nothing.
    ///
    /// At most one submission per [`CONFIRM_COOLDOWN`] is evaluated, so this
    /// path cannot be brute-forced like the check URL. Throttled submissions
    /// are not evaluated, do not count as attempts, and do not extend the
    /// window.
    ///
    /// # Errors
    /// - [`AppError::ConfirmThrottled`] if submitted within
    ///   [`CONFIRM_COOLDOWN`] of the last evaluated confirmation, carrying
    ///   the time left until the next one is evaluated.
    /// - [`AppError::WrongPassword`] if the password is not the secret.
    pub(crate) fn confirm(&mut self, password: &str, now: Timestamp) -> Result<bool, AppError> {
        if self.solved_at.is_none() {
            if let Some(last) = self.last_confirm_at {
                let retry_after = CONFIRM_COOLDOWN - now.duration_since(last);
                if retry_after.is_positive() {
                    return Err(AppError::ConfirmThrottled { retry_after });
                }
            }
            self.last_confirm_at = Some(now);
            self.attempts += 1;
        }

        if self.secret != password {
            return Err(AppError::WrongPassword);
        }
        let first = self.solved_at.is_none();
        if first {
            self.solved_at = Some(now);
        }
        Ok(first)
    }
}

#[cfg(test)]
mod tests {
    use hegel::generators;
    use jiff::SignedDuration;
    use rand::{RngExt, SeedableRng, distr::Alphanumeric, rngs::StdRng};

    use super::{AppError, CONFIRM_COOLDOWN, NUM_PASSWORDS, OFFSET, PASS_LEN, Player, Timestamp};
    use crate::testing::{timestamps, usernames, wrong_guess};

    fn player(tc: &hegel::TestCase) -> Player {
        Player::new(tc.draw(usernames()), tc.draw(timestamps()))
    }

    #[hegel::test(test_cases = 12)]
    fn password_file_contains_the_secret_exactly_once(tc: hegel::TestCase) {
        let player = player(&tc);
        let file = player.password_file().render();
        let hits = file.lines().filter(|l| *l == player.secret).count();
        assert_eq!(hits, 1);
    }

    #[hegel::test(test_cases = 12)]
    fn password_file_has_60000_lines_and_a_trailing_newline(tc: hegel::TestCase) {
        let file = player(&tc).password_file().render();
        assert!(file.ends_with('\n'));
        assert_eq!(file.lines().count(), NUM_PASSWORDS);
    }

    #[hegel::test(test_cases = 12)]
    fn every_password_is_32_alphanumeric_chars(tc: hegel::TestCase) {
        let file = player(&tc).password_file().render();
        for line in file.lines() {
            assert_eq!(line.len(), PASS_LEN);
            assert!(line.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }

    // Reference oracle: build the file the simple way, one String per line,
    // swapped, then joined. render() must match it byte for byte, or a
    // player's file would change under them mid-challenge.
    #[hegel::test(test_cases = 12)]
    fn password_file_matches_the_per_string_reference(tc: hegel::TestCase) {
        let player = player(&tc);

        let mut rng = StdRng::seed_from_u64(player.seed);
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
        let offset = (player.seed % (NUM_PASSWORDS - OFFSET) as u64) as usize + OFFSET;
        reference.swap(0, offset);
        reference.push(String::new());

        assert_eq!(player.password_file().render(), reference.join("\n"));
    }

    #[hegel::test(test_cases = 12)]
    fn password_file_is_deterministic(tc: hegel::TestCase) {
        let file = player(&tc).password_file();
        assert_eq!(file.render(), file.render());
    }

    #[hegel::test(test_cases = 12)]
    fn secret_is_never_in_the_first_15000_lines(tc: hegel::TestCase) {
        let player = player(&tc);
        let index = player
            .password_file()
            .render()
            .lines()
            .position(|l| l == player.secret)
            .unwrap();
        assert!((OFFSET..NUM_PASSWORDS).contains(&index));
    }

    #[hegel::test]
    fn player_is_deterministic_given_name_and_time(tc: hegel::TestCase) {
        let name = tc.draw(usernames());
        let now = tc.draw(timestamps());
        let a = Player::new(name.clone(), now);
        let b = Player::new(name, now);
        assert_eq!(a.seed, b.seed);
        assert_eq!(a.secret, b.secret);
        assert_eq!(a.joined_at, b.joined_at);
    }

    #[hegel::test]
    fn secret_is_32_alphanumeric_chars(tc: hegel::TestCase) {
        let player = player(&tc);
        assert_eq!(player.secret.len(), PASS_LEN);
        assert!(player.secret.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    // Regression guard: the original byte-sum seed gave anagram usernames the
    // same secret at the same instant. Hashing the username fixed it.
    #[test]
    fn anagram_usernames_get_different_secrets() {
        let now = Timestamp::UNIX_EPOCH;
        let abc = Player::new("abc".to_string(), now);
        let bca = Player::new("bca".to_string(), now);
        assert_ne!(abc.secret, bca.secret);
    }

    #[hegel::test]
    fn wrong_checks_count_but_never_solve(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let attempts = tc.draw(generators::integers::<u64>().max_value(20));
        let wrong = wrong_guess(&player.secret);
        for _ in 0..attempts {
            assert!(!player.check(&wrong));
        }
        assert_eq!(player.attempts, attempts);
        assert!(player.solved_at.is_none());
    }

    // The core of the feature: a script can loop right past the secret and
    // the challenge stays unsolved until it is confirmed.
    #[hegel::test]
    fn correct_checks_report_true_but_never_solve(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let secret = player.secret.clone();
        let hits = tc.draw(generators::integers::<u64>().min_value(1).max_value(20));
        for _ in 0..hits {
            assert!(player.check(&secret));
        }
        assert!(player.solved_at.is_none());
        assert_eq!(
            player.attempts, hits,
            "found-but-unconfirmed checks keep counting"
        );
    }

    #[hegel::test]
    fn correct_confirm_solves_exactly_once(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let joined = player.joined_at;
        let secret = player.secret.clone();
        let wrong = wrong_guess(&secret);

        let checks = tc.draw(generators::integers::<u64>().max_value(10));
        for _ in 0..checks {
            player.check(&wrong);
        }

        // Wrong confirmations spaced past the cooldown; each one counts.
        let confirms = tc.draw(generators::integers::<u64>().max_value(5));
        let mut now = joined;
        for _ in 0..confirms {
            let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
            now = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
            assert_eq!(player.confirm(&wrong, now), Err(AppError::WrongPassword));
        }

        let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
        let solved_at = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
        assert_eq!(player.confirm(&secret, solved_at), Ok(true));
        let standing = player.standing();
        assert_eq!(standing.attempts, checks + confirms + 1);
        // Span has no PartialEq; compare as fixed-unit durations.
        assert_eq!(
            standing.time_to_solve.unwrap().fieldwise(),
            (solved_at - joined).fieldwise()
        );

        // Checks and confirmations after solving report correctness but
        // never change the record, and the cooldown no longer applies.
        assert_eq!(player.confirm(&secret, solved_at), Ok(false));
        assert_eq!(
            player.confirm(&wrong, solved_at),
            Err(AppError::WrongPassword)
        );
        assert!(player.check(&secret));
        assert!(!player.check(&wrong));
        assert_eq!(player.attempts, checks + confirms + 1);
        assert_eq!(player.solved_at, Some(solved_at));
    }

    // The throttle gate sits in front of evaluation: inside the window even
    // the secret is rejected, nothing is counted, and the error carries
    // exactly the time left.
    #[hegel::test]
    fn confirms_within_the_cooldown_are_throttled_and_uncounted(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let joined = player.joined_at;
        let secret = player.secret.clone();
        let wrong = wrong_guess(&secret);

        assert_eq!(player.confirm(&wrong, joined), Err(AppError::WrongPassword));

        let within = tc.draw(generators::integers::<i64>().min_value(0).max_value(9_999));
        let elapsed = SignedDuration::from_millis(within);
        let again = joined.checked_add(elapsed).unwrap();
        assert_eq!(
            player.confirm(&secret, again),
            Err(AppError::ConfirmThrottled {
                retry_after: CONFIRM_COOLDOWN - elapsed
            })
        );
        assert!(player.solved_at.is_none());
        assert_eq!(player.attempts, 1);
    }

    // The clock can step backwards (NTP). A confirmation timestamped before
    // the last evaluated one sits inside the window and is throttled, no
    // matter how far back the rewind goes; the gate heals on its own once
    // the clock passes the old mark plus the cooldown.
    #[hegel::test]
    fn confirms_with_a_rewound_clock_are_throttled(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let joined = player.joined_at;
        let secret = player.secret.clone();
        let wrong = wrong_guess(&secret);

        assert_eq!(player.confirm(&wrong, joined), Err(AppError::WrongPassword));

        let rewind = tc.draw(generators::integers::<i64>().min_value(1).max_value(86_400));
        let earlier = joined
            .checked_sub(SignedDuration::from_secs(rewind))
            .unwrap();
        assert!(matches!(
            player.confirm(&secret, earlier),
            Err(AppError::ConfirmThrottled { .. })
        ));
        assert!(player.solved_at.is_none());
        assert_eq!(player.attempts, 1);
    }

    // Checks and confirmations interleave in the wild. Every evaluated
    // guess counts exactly once regardless of the order, and wrong guesses
    // never solve.
    #[hegel::test]
    fn interleaved_wrong_guesses_each_count_once(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let wrong = wrong_guess(&player.secret);

        let guesses = tc.draw(generators::integers::<u64>().max_value(20));
        let mut now = player.joined_at;
        for _ in 0..guesses {
            if tc.draw(generators::booleans()) {
                assert!(!player.check(&wrong));
            } else {
                let gap = tc.draw(generators::integers::<i64>().min_value(10).max_value(3600));
                now = now.checked_add(SignedDuration::from_secs(gap)).unwrap();
                assert_eq!(player.confirm(&wrong, now), Err(AppError::WrongPassword));
            }
        }
        assert_eq!(player.attempts, guesses);
        assert!(player.solved_at.is_none());
    }

    // Throttled submissions must not extend the window, or a hammering
    // script could lock a player out of confirming forever.
    #[hegel::test]
    fn throttled_confirms_do_not_extend_the_cooldown_window(tc: hegel::TestCase) {
        let mut player = player(&tc);
        let joined = player.joined_at;
        let secret = player.secret.clone();
        let wrong = wrong_guess(&secret);

        assert_eq!(player.confirm(&wrong, joined), Err(AppError::WrongPassword));
        let hammers = tc.draw(generators::integers::<i64>().min_value(1).max_value(9));
        for s in 1..=hammers {
            let at = joined.checked_add(SignedDuration::from_secs(s)).unwrap();
            assert!(matches!(
                player.confirm(&wrong, at),
                Err(AppError::ConfirmThrottled { .. })
            ));
        }

        // The window is measured from the last *evaluated* confirmation, so
        // it ends exactly one cooldown after the first submission.
        let at = joined.checked_add(CONFIRM_COOLDOWN).unwrap();
        assert_eq!(player.confirm(&secret, at), Ok(true));
        assert_eq!(player.attempts, 2);
    }
}
