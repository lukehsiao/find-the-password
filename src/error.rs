use topcoat::router::StatusCode;

/// Mistakes a player can make through the forms.
///
/// The `#[error]` strings are the user-facing messages, rendered directly
/// on the page that re-renders the failed form.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppError {
    #[error(
        "Username must be 3-32 characters: ASCII letters, digits, hyphens, periods, or underscores."
    )]
    InvalidUsername,
    #[error("That username is already taken. Try another.")]
    UsernameTaken,
    #[error("That's not the password. Keep hunting!")]
    WrongPassword,
    #[error("Whoa, slow down! You can only confirm once every 10 seconds.")]
    ConfirmThrottled,
}

impl AppError {
    /// The HTTP status the re-rendered form travels with, so anyone
    /// scripting the forms can tell a mistake from success without
    /// scraping the page. Every variant is the player's mistake, so every
    /// status is in the 4xx range.
    #[must_use]
    pub fn status(self) -> StatusCode {
        match self {
            AppError::InvalidUsername | AppError::WrongPassword => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::UsernameTaken => StatusCode::CONFLICT,
            AppError::ConfirmThrottled => StatusCode::TOO_MANY_REQUESTS,
        }
    }
}
