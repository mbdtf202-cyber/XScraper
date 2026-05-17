use serde_json::json;
use tempfile::tempdir;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use xscraper::api::{Api, ApiConfig};
use xscraper::error::XScraperError;
use xscraper::gql::OP_SEARCH_TIMELINE;
use xscraper::pool::{AccountsPool, AddAccount};

fn locked(pool: &AccountsPool) -> Vec<String> {
    pool.get_all()
        .unwrap()
        .into_iter()
        .filter(|account| account.locks.contains_key("SearchTimeline"))
        .map(|account| account.username)
        .collect()
}

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

#[tokio::test]
async fn api_releases_account_lock_when_request_errors() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
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
    pool.set_active("user1", true).unwrap();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/i/api/graphql/{OP_SEARCH_TIMELINE}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>blocked</html>"))
        .expect(1)
        .mount(&server)
        .await;

    let api = Api::with_config(pool.clone(), ApiConfig { proxy: None, base_url: server.uri() });
    let err = api.search_raw("foo", 10, None).await.unwrap_err();

    assert!(
        matches!(err, XScraperError::RequestAborted(message) if message.contains("invalid JSON response"))
    );
    assert!(locked(&pool).is_empty());
}

#[tokio::test]
async fn api_sends_graphql_requests_as_post_json() {
    unsafe { std::env::set_var("XSCRAPER_DISABLE_XCLID", "1") };
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
    pool.set_active("user1", true).unwrap();

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/i/api/graphql/{OP_SEARCH_TIMELINE}")))
        .and(body_json(json!({
            "variables": {
                "rawQuery": "rust",
                "count": 20,
                "product": "Latest",
                "querySource": "typed_query",
                "withGrokTranslatedBio": false,
                "withQuickPromoteEligibilityTweetFields": false
            },
            "features": xscraper::gql::default_features(),
            "fieldToggles": {
                "withPayments": false,
                "withAuxiliaryUserLabels": false,
                "withArticleRichContentState": false,
                "withArticlePlainText": false,
                "withArticleSummaryText": false,
                "withArticleVoiceOver": false,
                "withGrokAnalyze": false,
                "withDisallowedReplyControls": false
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "search_by_raw_query": {
                    "search_timeline": {
                        "timeline": {
                            "instructions": [
                                {
                                    "type": "TimelineAddEntries",
                                    "entries": [
                                        {
                                            "entryId": "tweet-1",
                                            "content": {
                                                "itemContent": {
                                                    "tweet_results": {
                                                        "result": {
                                                            "__typename": "Tweet",
                                                            "rest_id": "1",
                                                            "legacy": {
                                                                "full_text": "rust",
                                                                "created_at": "Mon Jan 01 00:00:00 +0000 2024",
                                                                "user_id_str": "2"
                                                            },
                                                            "core": {
                                                                "user_results": {
                                                                    "result": {
                                                                        "__typename": "User",
                                                                        "rest_id": "2",
                                                                        "legacy": {
                                                                            "screen_name": "rustacean",
                                                                            "name": "Rustacean",
                                                                            "created_at": "Mon Jan 01 00:00:00 +0000 2024"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    ]
                                }
                            ]
                        }
                    }
                }
            }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let api = Api::with_config(pool.clone(), ApiConfig { proxy: None, base_url: server.uri() });
    let pages = api.search_raw("rust", 1, None).await.unwrap();

    assert_eq!(pages.len(), 1);
    assert!(locked(&pool).is_empty());
}
