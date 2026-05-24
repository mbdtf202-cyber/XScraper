use serde_json::json;
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use xscraper::pool::{AccountsPool, AddAccount};
use xscraper::queue_client::QueueClient;

fn single_account_pool() -> (tempfile::TempDir, AccountsPool) {
    let dir = tempdir().unwrap();
    let pool = AccountsPool::new(dir.path().join("test.db")).with_raise_when_no_account(true);
    pool.add_account(AddAccount {
        username: "user1".into(),
        password: "pass1".into(),
        email: "email@example.com".into(),
        email_password: "email_pass".into(),
        proxy: Some("http://proxy-a:8080".into()),
        ..AddAccount::default()
    })
    .unwrap();
    pool.set_active("user1", true).unwrap();
    (dir, pool)
}

#[tokio::test]
async fn account_health_records_rate_limit_events_and_score() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = single_account_pool();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("x-rate-limit-remaining", "0")
                .insert_header("x-rate-limit-reset", "1893456000")
                .set_body_json(json!({"errors": [{"code": 88, "message": "Rate limit exceeded"}]})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    let _ = session.get("/api", Vec::new()).await.unwrap_err();

    let report = pool.health_report().unwrap();
    let account = report.accounts.iter().find(|account| account.username == "user1").unwrap();
    assert!(account.health_score < 100);
    assert_eq!(account.event_counts.get("rate_limited").copied(), Some(1));
    assert!(account.reasons.iter().any(|reason| reason.contains("rate")));
}

#[tokio::test]
async fn proxy_health_records_proxy_identity_from_account_binding() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
    let (_dir, pool) = single_account_pool();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;

    let client = QueueClient::new(pool.clone(), "SearchTimeline").with_base_url(server.uri());
    let mut session = client.open().await.unwrap();
    let _ = session.get("/api", Vec::new()).await.unwrap().unwrap();
    session.close().await.unwrap();

    let report = pool.health_report().unwrap();
    let proxy = report.proxies.iter().find(|proxy| proxy.proxy.contains("proxy-a")).unwrap();
    assert!(proxy.score >= 100);
    assert_eq!(proxy.successes, 1);
}
