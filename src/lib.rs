pub mod account;
pub mod analysis;
pub mod api;
pub mod browser_probe;
pub mod cli;
pub mod config;
pub mod diagnostics;
pub mod drift;
pub mod error;
pub mod evidence;
pub mod fetch_profile;
pub mod gql;
pub mod imap;
pub mod jobs;
pub mod lists;
pub mod login;
pub mod models;
pub mod operations;
pub mod parser;
pub mod pool;
pub mod queue_client;
pub mod storage;
pub mod utils;
pub mod xclid;

pub use account::Account;
pub use api::{Api, ApiConfig};
pub use error::{Result, XScraperError};
pub use models::{ListInfo, Trend, Tweet, User};
pub use pool::AccountsPool;

pub fn init_tracing(debug: bool) {
    let default_level = if debug { "debug" } else { "info" };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| default_level.into());

    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_target(false).try_init();
}
