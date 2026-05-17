use crate::api::ApiConfig;
use crate::error::{Result, XScraperError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XScraperConfig {
    pub db: PathBuf,
    pub proxy: Option<String>,
    pub base_url: String,
    pub raise_when_no_account: bool,
}

impl Default for XScraperConfig {
    fn default() -> Self {
        Self {
            db: PathBuf::from("accounts.db"),
            proxy: None,
            base_url: "https://x.com".into(),
            raise_when_no_account: false,
        }
    }
}

impl XScraperConfig {
    pub fn from_json_file(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let raw = std::fs::read_to_string(&path)
            .map_err(|source| XScraperError::io(path.clone(), source))?;
        serde_json::from_str(&raw).map_err(Into::into)
    }

    pub fn api_config(&self) -> ApiConfig {
        ApiConfig { proxy: self.proxy.clone(), base_url: self.base_url.clone() }
    }
}
