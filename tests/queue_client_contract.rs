use tempfile::{TempDir, tempdir};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use xscraper::error::XScraperError;
use xscraper::pool::{AccountsPool, AddAccount};
use xscraper::queue_client::QueueClient;

fn pool() -> (TempDir, AccountsPool) {
    let dir = tempdir().unwrap();
    let db = dir.path().join("test.db");
    let pool = AccountsPool::new(db);
    for idx in 1..=2 {
        pool.add_account(AddAccount {
            username: format!("user{idx}"),
            password: format!("pass{idx}"),
            email: format!("email{idx}@example.com"),
            email_password: format!("email_pass{idx}"),
            ..AddAccount::default()
        })
        .unwrap();
        pool.set_active(&format!("user{idx}"), true).unwrap();
    }
    (dir, pool)
}

fn locked(pool: &AccountsPool) -> Vec<String> {
    pool.get_all()
        .unwrap()
        .into_iter()
        .filter(|account| account.locks.contains_key("SearchTimeline"))
        .map(|account| account.username)
        .collect()
}

#[tokio::test]
async fn locks_account_on_open_and_unlocks_on_close() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = pool();
    let client = QueueClient::new(pool.clone(), "SearchTimeline");

    assert!(locked(&pool).is_empty());
    let mut session = client.open().await.unwrap();
    assert_eq!(locked(&pool), vec!["user1"]);
    session.close().await.unwrap();
    assert!(locked(&pool).is_empty());
}

#[tokio::test]
async fn does_not_switch_account_on_successful_requests() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = pool();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "ok": true })))
        .expect(2)
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    let first_locked = locked(&pool);
    assert_eq!(first_locked, vec!["user1"]);

    for _ in 0..2 {
        let response = session.get("/api", Vec::new()).await.unwrap().unwrap();
        assert_eq!(response.account_username, "user1");
        assert_eq!(response.value["ok"], true);
        assert_eq!(locked(&pool), first_locked);
    }

    session.close().await.unwrap();
    assert!(locked(&pool).is_empty());
}

#[tokio::test]
async fn switches_account_on_http_error_and_keeps_failed_account_locked() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = pool();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({})))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "ok": 2 })))
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    assert_eq!(locked(&pool), vec!["user1"]);

    let response = session.get("/api", Vec::new()).await.unwrap().unwrap();
    assert_eq!(response.account_username, "user2");
    assert_eq!(response.value["ok"], 2);
    assert_eq!(locked(&pool), vec!["user1", "user2"]);

    session.close().await.unwrap();
    assert_eq!(locked(&pool), vec!["user1"]);
}

#[tokio::test]
async fn repeated_retryable_errors_return_error_instead_of_empty_response() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let dir = tempdir().unwrap();
    let db = dir.path().join("test.db");
    let pool = AccountsPool::new(db);
    pool.add_account(AddAccount {
        username: "user1".into(),
        password: "pass1".into(),
        email: "email1@example.com".into(),
        email_password: "email_pass1".into(),
        ..AddAccount::default()
    })
    .unwrap();
    pool.set_active("user1", true).unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({})))
        .expect(3)
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    let err = session.get("/api", Vec::new()).await.unwrap_err();

    assert!(matches!(err, XScraperError::RequestAborted(_)));
    assert_eq!(locked(&pool), vec!["user1"]);
}

#[tokio::test]
async fn successful_non_json_response_is_reported_as_error() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = pool();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>blocked</html>"))
        .expect(1)
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    let err = session.get("/api", Vec::new()).await.unwrap_err();

    assert!(
        matches!(err, XScraperError::RequestAborted(message) if message.contains("invalid JSON response"))
    );
    session.close().await.unwrap();
    assert!(locked(&pool).is_empty());
}
