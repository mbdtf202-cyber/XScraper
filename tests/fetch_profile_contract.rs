use std::time::Duration;
use xscraper::fetch_profile::FetchProfile;

#[test]
fn fetch_profile_applies_timeout_proxy_headers_and_request_id() {
    let profile = FetchProfile::new()
        .with_timeout(Duration::from_secs(12))
        .with_proxy(Some("http://proxy-a:8080".to_string()))
        .with_header("x-xscraper-test", "1")
        .with_request_id("req-1");

    assert_eq!(profile.timeout(), Duration::from_secs(12));
    assert_eq!(profile.proxy(), Some("http://proxy-a:8080"));
    assert_eq!(profile.headers().get("x-xscraper-test").unwrap(), "1");
    assert_eq!(profile.request_id(), Some("req-1"));

    let client = profile.client_for_base_url("http://127.0.0.1:1").unwrap();
    let _ = client;
}
