use chrono::{DateTime, Utc};
use xscraper::analysis::{AnalysisWindow, analyze_list_tweets, analyze_tweets, normalize_login};
use xscraper::lists::normalize_list_target;
use xscraper::models::{Media, Tweet};

#[test]
fn normalize_login_accepts_urls_and_handles() {
    assert_eq!(normalize_login("https://x.com/0xSunNFT"), "0xSunNFT");
    assert_eq!(normalize_login("https://twitter.com/gemfindercalls/"), "gemfindercalls");
    assert_eq!(normalize_login("@user"), "user");
}

#[test]
fn normalize_list_target_accepts_ids_urls_and_slugs() {
    assert_eq!(normalize_list_target("123456").unwrap().id().unwrap(), "123456");
    assert_eq!(
        normalize_list_target("https://x.com/i/lists/123456").unwrap().id().unwrap(),
        "123456"
    );
    let slug = normalize_list_target("https://x.com/xdev/lists/rust-team").unwrap();
    assert_eq!(slug.owner_and_slug().unwrap(), ("xdev", "rust-team"));
}

#[test]
fn analyze_tweets_summarizes_engagement_terms_and_latest() {
    let first = tweet(1, "2026-05-17T00:00:00Z", "Trading $LAB signal #LABUSDT", 10);
    let duplicate = tweet(1, "2026-05-17T00:00:00Z", "Duplicate", 999);
    let second = tweet(2, "2026-05-16T00:00:00Z", "Trading risk framework", 3);

    let report = analyze_tweets(
        "gemfindercalls".into(),
        None,
        AnalysisWindow {
            days: 7,
            since: "2026-05-11".into(),
            until: "2026-05-19".into(),
            time_zone: "UTC".into(),
        },
        vec![first, duplicate, second],
    );

    assert_eq!(report.fetched_count, 3);
    assert_eq!(report.tweet_count, 2);
    assert_eq!(report.engagement_sum, 19);
    assert_eq!(report.cashtags[0].term, "$LAB");
    assert_eq!(report.hashtags[0].term, "#LABUSDT");
    assert_eq!(report.top_engagement[0].id, "1");
    assert_eq!(report.latest[0].id, "1");
}

#[test]
fn analyze_list_tweets_keeps_list_identity_and_reuses_tweet_metrics() {
    let first = tweet(1, "2026-05-17T00:00:00Z", "Trading $LAB signal #LABUSDT", 10);
    let second = tweet(2, "2026-05-16T00:00:00Z", "Trading risk framework", 3);

    let report = analyze_list_tweets(
        "123456".into(),
        None,
        AnalysisWindow {
            days: 7,
            since: "2026-05-11".into(),
            until: "2026-05-19".into(),
            time_zone: "UTC".into(),
        },
        vec![first, second],
    );

    assert_eq!(report.list, "123456");
    assert_eq!(report.tweet_count, 2);
    assert_eq!(report.engagement_sum, 19);
    assert_eq!(report.top_engagement[0].id, "1");
}

fn tweet(id: u64, date: &str, text: &str, likes: i64) -> Tweet {
    Tweet {
        id,
        id_str: id.to_string(),
        url: format!("https://x.com/test/status/{id}"),
        date: DateTime::parse_from_rfc3339(&date.replace('Z', "+00:00"))
            .unwrap()
            .with_timezone(&Utc),
        user: serde_json::from_value(serde_json::json!({
            "id": 1,
            "id_str": "1",
            "url": "https://x.com/test",
            "username": "test",
            "displayname": "Test",
            "rawDescription": "",
            "created": "2026-01-01T00:00:00Z",
            "followersCount": 0,
            "friendsCount": 0,
            "statusesCount": 0,
            "favouritesCount": 0,
            "listedCount": 0,
            "mediaCount": 0,
            "location": "",
            "profileImageUrl": "",
            "profileBannerUrl": null,
            "protected": false,
            "verified": false,
            "blue": false,
            "blueType": null,
            "descriptionLinks": [],
            "pinnedIds": [],
            "_type": "xscraper.User"
        }))
        .unwrap(),
        lang: "en".into(),
        raw_content: text.into(),
        reply_count: 1,
        retweet_count: 1,
        like_count: likes,
        quote_count: 1,
        bookmarked_count: 0,
        conversation_id: id,
        conversation_id_str: id.to_string(),
        hashtags: vec!["LABUSDT".into()],
        cashtags: vec!["LAB".into()],
        mentioned_users: vec![],
        links: vec![],
        media: Media::default(),
        view_count: Some(100),
        retweeted_tweet: None,
        quoted_tweet: None,
        place: None,
        coordinates: None,
        in_reply_to_tweet_id: None,
        in_reply_to_tweet_id_str: None,
        in_reply_to_user: None,
        source: None,
        source_url: None,
        source_label: None,
        card: None,
        possibly_sensitive: None,
        object_type: "xscraper.Tweet".into(),
    }
}
