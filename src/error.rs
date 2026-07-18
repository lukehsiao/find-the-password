use leptos::server_fn::{
    codec::JsonEncoding,
    error::{FromServerFnError, ServerFnErrorErr},
};
use serde::{Deserialize, Serialize};

/// Errors surfaced to the player across the server function boundary.
///
/// The `#[error]` strings are the user-facing messages, rendered directly by
/// the UI. They travel the wire as serde-tagged variants (JSON), so changing
/// a message never breaks decoding and the client never parses error strings.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum AppError {
    #[error("Username must be 3-32 ASCII letters or digits, with no spaces or symbols.")]
    InvalidUsername,
    #[error("That username is already taken. Try another.")]
    UsernameTaken,
    #[error("No user with that username.")]
    UserNotFound,
    #[error("That's not the password. Keep hunting!")]
    WrongPassword,
    #[error("Whoa, slow down! You can only confirm once every 10 seconds.")]
    ConfirmThrottled,
    /// Framework-level failures (network, serialization, ...) funnel here so
    /// every server function can use `AppError` as its only error type.
    #[error("{0}")]
    ServerFn(ServerFnErrorErr),
}

#[cfg(feature = "ssr")]
impl AppError {
    /// The HTTP status an error response travels with.
    ///
    /// `server_fn` flattens every application error to a blanket 500 on the
    /// wire; the server functions override that through `ResponseOptions`
    /// so client mistakes stay in the 4xx range for anyone scripting the
    /// endpoints.
    #[must_use]
    pub fn status(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;

        match self {
            AppError::InvalidUsername | AppError::WrongPassword => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::UsernameTaken => StatusCode::CONFLICT,
            AppError::UserNotFound => StatusCode::NOT_FOUND,
            AppError::ConfirmThrottled => StatusCode::TOO_MANY_REQUESTS,
            AppError::ServerFn(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl FromServerFnError for AppError {
    type Encoder = JsonEncoding;

    fn from_server_fn_error(value: ServerFnErrorErr) -> Self {
        Self::ServerFn(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leptos::server_fn::error::FromServerFnError;

    #[test]
    fn app_error_survives_wire_roundtrip() {
        let errors = [
            AppError::InvalidUsername,
            AppError::UsernameTaken,
            AppError::UserNotFound,
            AppError::WrongPassword,
            AppError::ConfirmThrottled,
            AppError::ServerFn(ServerFnErrorErr::ServerError("boom".to_string())),
        ];
        for error in errors {
            assert_eq!(error, AppError::de(error.ser()));
        }
    }

    // Only framework failures may report as server errors; everything a
    // player can trigger is their mistake and must stay in the 4xx range.
    #[cfg(feature = "ssr")]
    #[test]
    fn every_player_error_is_a_client_error() {
        let player_errors = [
            AppError::InvalidUsername,
            AppError::UsernameTaken,
            AppError::UserNotFound,
            AppError::WrongPassword,
            AppError::ConfirmThrottled,
        ];
        for error in player_errors {
            assert!(error.status().is_client_error(), "{error:?}");
        }
        let framework = AppError::ServerFn(ServerFnErrorErr::ServerError("boom".to_string()));
        assert!(framework.status().is_server_error());
    }

    #[test]
    fn framework_errors_funnel_into_the_server_fn_variant() {
        let err = AppError::from_server_fn_error(ServerFnErrorErr::ServerError("boom".to_string()));
        assert_eq!(
            err,
            AppError::ServerFn(ServerFnErrorErr::ServerError("boom".to_string()))
        );
    }
}
