use std::sync::{Arc, Mutex, PoisonError};

use jiff::{SignedDuration, Timestamp};

/// The wall clock the handlers stamp joins and confirmations with.
///
/// [`Clock::system`] reads the real time. [`Clock::null`] is the same
/// clock with the outside world switched off: frozen at a fixed instant
/// until [`Clock::advance`] moves it, so everything that depends on time
/// (the confirmation cooldown, solve durations, password seeds) is
/// deterministic. Clones share one source, so a test keeps a clone and
/// moves time under a running router.
#[derive(Debug, Clone)]
pub struct Clock(Source);

#[derive(Debug, Clone)]
enum Source {
    System,
    Frozen(Arc<Mutex<Timestamp>>),
}

impl Clock {
    /// Where a nulled clock starts: the Back to the Future time-travel
    /// date, absurd enough that a test leaking it into an assertion is
    /// obviously relying on the default rather than on a time it chose.
    pub const NULL_START: Timestamp = Timestamp::constant(499_137_660, 0);

    #[must_use]
    pub fn system() -> Self {
        Self(Source::System)
    }

    /// A clock frozen at [`Clock::NULL_START`].
    #[must_use]
    pub fn null() -> Self {
        Self(Source::Frozen(Arc::new(Mutex::new(Self::NULL_START))))
    }

    #[must_use]
    pub fn now(&self) -> Timestamp {
        match &self.0 {
            Source::System => Timestamp::now(),
            Source::Frozen(now) => *now.lock().unwrap_or_else(PoisonError::into_inner),
        }
    }

    /// Move a nulled clock forward by `by`.
    ///
    /// # Panics
    /// On the system clock, which cannot be moved, or if the result leaves
    /// the range `Timestamp` supports. Both are test bugs.
    pub fn advance(&self, by: SignedDuration) {
        let Source::Frozen(now) = &self.0 else {
            panic!("only a nulled clock can be advanced");
        };
        let mut now = now.lock().unwrap_or_else(PoisonError::into_inner);
        *now = now
            .checked_add(by)
            .expect("advanced clock stays within Timestamp's range");
    }
}
