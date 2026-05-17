use crate::error::{Result, XScraperError};
use chrono::{DateTime, Utc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, thiserror::Error)]
#[error("email login error: {message}")]
pub struct EmailLoginError {
    pub message: String,
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("email code timeout: {message}")]
pub struct EmailCodeTimeoutError {
    pub message: String,
}

pub fn imap_domain_for_email(email: &str) -> String {
    let domain = email.split('@').nth(1).unwrap_or_default();
    let mapped = match domain {
        "icloud.com" => "imap.mail.me.com",
        "outlook.com" | "hotmail.com" => "imap-mail.outlook.com",
        "yahoo.com" => "imap.mail.yahoo.com",
        other => return format!("imap.{other}"),
    };
    mapped.to_string()
}

pub async fn imap_get_email_code(
    email: &str,
    password: &str,
    min_time: DateTime<Utc>,
) -> Result<String> {
    let email = email.to_string();
    let password = password.to_string();
    tokio::task::spawn_blocking(move || imap_get_email_code_blocking(&email, &password, min_time))
        .await
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?
}

fn imap_get_email_code_blocking(
    email: &str,
    password: &str,
    min_time: DateTime<Utc>,
) -> Result<String> {
    let host = imap_domain_for_email(email);
    let timeout = Duration::from_secs(env_u64("XSCRAPER_IMAP_TIMEOUT_SECONDS", 60));
    let poll = Duration::from_secs(env_u64("XSCRAPER_IMAP_POLL_SECONDS", 5));
    let started = Instant::now();

    while started.elapsed() <= timeout {
        if let Some(code) = fetch_latest_code(&host, email, password, min_time)? {
            return Ok(code);
        }
        std::thread::sleep(poll);
    }

    Err(XScraperError::LoginFlow("email code timeout".into()))
}

fn fetch_latest_code(
    host: &str,
    email: &str,
    password: &str,
    min_time: DateTime<Utc>,
) -> Result<Option<String>> {
    let client = imap::ClientBuilder::new(host, 993)
        .connect()
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let mut session = client
        .login(email, password)
        .map_err(|(error, _)| XScraperError::LoginFlow(error.to_string()))?;
    session.select("INBOX").map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let messages = session
        .fetch("1:*", "(RFC822 INTERNALDATE)")
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;

    let mut found = None;
    for message in messages.iter().rev() {
        if let Some(date) = message.internal_date()
            && date.with_timezone(&Utc) < min_time
        {
            continue;
        }
        let Some(body) = message.body() else {
            continue;
        };
        let text = String::from_utf8_lossy(body);
        if let Some(code) = extract_email_code(&text) {
            found = Some(code);
            break;
        }
    }
    let _ = session.logout();
    Ok(found)
}

pub fn extract_email_code(text: &str) -> Option<String> {
    let mut digits = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
            if digits.len() == 6 {
                return Some(digits);
            }
        } else {
            digits.clear();
        }
    }
    None
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key).ok().and_then(|value| value.parse().ok()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{extract_email_code, imap_domain_for_email};

    #[test]
    fn maps_common_imap_domains() {
        assert_eq!(imap_domain_for_email("a@yahoo.com"), "imap.mail.yahoo.com");
        assert_eq!(imap_domain_for_email("a@icloud.com"), "imap.mail.me.com");
        assert_eq!(imap_domain_for_email("a@outlook.com"), "imap-mail.outlook.com");
        assert_eq!(imap_domain_for_email("a@example.com"), "imap.example.com");
    }

    #[test]
    fn extracts_six_digit_code() {
        assert_eq!(extract_email_code("Your code is 123456."), Some("123456".into()));
        assert_eq!(extract_email_code("no code"), None);
    }
}
