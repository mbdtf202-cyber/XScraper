use serde_json::json;
use tempfile::tempdir;
use xscraper::api::Api;
use xscraper::error::XScraperError;
use xscraper::pool::{AccountsPool, AddAccount};

#[test]
fn gql_params_allow_kv_count_override_for_generator_methods() {
    let dir = tempdir().unwrap();
    let api = Api::new(AccountsPool::new(dir.path().join("test.db")));
    for (method, arg) in [
        ("search", "rust"),
        ("tweet_replies", "123"),
        ("retweeters", "123"),
        ("followers", "123"),
        ("following", "123"),
        ("user_tweets", "123"),
        ("user_tweets_and_replies", "123"),
        ("list_timeline", "123"),
        ("trends", "sport"),
    ] {
        let request = api.operation_request(method, arg, Some(json!({ "count": 100 }))).unwrap();
        assert_eq!(request.variables["count"], 100, "{method}");
        assert!(!request.op.is_empty(), "{method}");
        assert!(!request.queue.is_empty(), "{method}");
    }
}

#[tokio::test]
async fn raise_when_no_account_reports_empty_pool() {
    let dir = tempdir().unwrap();
    let pool = AccountsPool::new(dir.path().join("test.db")).with_raise_when_no_account(true);
    let api = Api::new(pool);

    let err = api.search("foo", 10, None).await.unwrap_err();
    assert!(matches!(err, XScraperError::NoAccount { .. }));

    let err = api.user_by_id(123, None).await.unwrap_err();
    assert!(matches!(err, XScraperError::NoAccount { .. }));
}

#[tokio::test]
async fn inactive_account_does_not_satisfy_queue() {
    let dir = tempdir().unwrap();
    let pool = AccountsPool::new(dir.path().join("test.db")).with_raise_when_no_account(true);
    pool.add_account(AddAccount {
        username: "user1".into(),
        password: "pass1".into(),
        email: "email@example.com".into(),
        email_password: "email_pass".into(),
        ..AddAccount::default()
    })
    .unwrap();
    let api = Api::new(pool);
    let err = api.search("foo", 10, None).await.unwrap_err();
    assert!(matches!(err, XScraperError::NoAccount { .. }));
}
