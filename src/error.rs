use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, XScraperError>;

#[derive(Debug, thiserror::Error)]
pub enum XScraperError {
    #[error("account {0} already exists")]
    AccountExists(String),

    #[error("account {0} not found")]
    AccountNotFound(String),

    #[error("no active account available for queue {queue}")]
    NoAccount { queue: String },

    #[error("invalid account line format: {0}")]
    InvalidLineFormat(String),

    #[error("invalid account line: {0}")]
    InvalidAccountLine(String),

    #[error("invalid cookie value: {0}")]
    InvalidCookie(String),

    #[error(
        "login by password/email is not implemented in the Rust rewrite; add cookies containing ct0/auth_token or import an exported cookie jar"
    )]
    LoginFlowNotImplemented,

    #[error("login flow error: {0}")]
    LoginFlow(String),

    #[error("request aborted: {0}")]
    RequestAborted(String),

    #[error("parse error at {path}: {message}")]
    Parse { path: String, message: String },

    #[error("configuration error: {0}")]
    Config(String),

    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),

    #[error(transparent)]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Chrono(#[from] chrono::ParseError),
}

impl XScraperError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}
