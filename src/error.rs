/// Every failure the SDK reports. `status` is the HTTP status, or `0` when no answer came back.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file is larger than the gateway accepts (10 MB decoded).
    #[error("a file may be at most 10 MB")]
    FileTooLarge,
    /// CogniCloud answered with an error. `code` is its stable error code.
    #[error("CogniCloud error {status} {code}: {message}")]
    Api {
        status: u16,
        code: String,
        message: String,
    },
    /// The request did not finish within the configured timeout.
    #[error("the request timed out")]
    Timeout,
    /// The request never reached CogniCloud.
    #[error("the request could not reach CogniCloud: {0}")]
    Network(#[source] reqwest::Error),
    /// The client was built without something it needs.
    #[error("invalid configuration: {0}")]
    Config(String),
    /// An environment variable the caller read was missing or not Unicode.
    #[error(transparent)]
    Env(#[from] std::env::VarError),
    /// CogniCloud's answer did not have the expected shape.
    #[error("unexpected response: {0}")]
    Decode(String),
}

impl Error {
    /// HTTP status, or `0` when no answer came back.
    pub fn status(&self) -> u16 {
        match self {
            Error::Api { status, .. } => *status,
            _ => 0,
        }
    }

    /// CogniCloud's stable error code, or a local code such as `timeout` or `network_error`.
    pub fn code(&self) -> &str {
        match self {
            Error::Api { code, .. } => code,
            Error::Timeout => "timeout",
            Error::Network(_) => "network_error",
            Error::Config(_) => "config",
            Error::FileTooLarge => "file_too_large",
            Error::Env(_) => "env",
            Error::Decode(_) => "decode",
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
