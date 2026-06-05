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
    #[error(
        "Username must be 3-32 ASCII letters or digits, with no spaces or symbols."
    )]
    InvalidUsername,
    #[error("That username is already taken. Try another.")]
    UsernameTaken,
    #[error("No user with that username.")]
    UserNotFound,
    /// Framework-level failures (network, serialization, ...) funnel here so
    /// every server function can use `AppError` as its only error type.
    #[error("{0}")]
    ServerFn(ServerFnErrorErr),
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
            AppError::ServerFn(ServerFnErrorErr::ServerError("boom".to_string())),
        ];
        for error in errors {
            assert_eq!(error, AppError::de(error.ser()));
        }
    }
}
