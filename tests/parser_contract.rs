use serde_json::Value;
use xscraper::models::{Card, Tweet, User, UserRef};
use xscraper::parser::{parse_trends, parse_tweet, parse_tweets, parse_users};

mod support {
    pub mod sample_payloads;
}

use support::sample_payloads::{search_payload, trend_payload, tweet_payload, user_payload};

fn check_user_ref(user: &UserRef) {
    assert_eq!(user.id.to_string(), user.id_str);
    assert!(!user.username.is_empty());
    assert!(!user.displayname.is_empty());
    assert_eq!(user.object_type, "xscraper.UserRef");
}

fn check_user(user: &User) {
    assert_eq!(user.id.to_string(), user.id_str);
    assert!(!user.username.is_empty());
    assert!(!user.description_links.iter().any(|link| link.url.is_empty()));
    assert!(user.pinned_ids.iter().all(|id| *id > 0));
    assert_eq!(user.object_type, "xscraper.User");

    let json = serde_json::to_string(user).unwrap();
    assert!(json.contains(&user.id_str));
}

fn check_tweet(tweet: &Tweet) {
    assert_eq!(tweet.id.to_string(), tweet.id_str);
    assert!(tweet.url.contains(&tweet.id_str));
    assert_eq!(tweet.conversation_id.to_string(), tweet.conversation_id_str);
    assert_eq!(tweet.object_type, "xscraper.Tweet");
    assert!(tweet.bookmarked_count >= 0);

    if let Some(reply_id) = tweet.in_reply_to_tweet_id {
        assert_eq!(Some(reply_id.to_string()), tweet.in_reply_to_tweet_id_str);
    }

    if let Some(user) = &tweet.in_reply_to_user {
        check_user_ref(user);
    }
    for user in &tweet.mentioned_users {
        check_user_ref(user);
    }
    for video in &tweet.media.videos {
        assert!(!video.thumbnail_url.is_empty());
        assert!(video.duration > 0);
        for variant in &video.variants {
            assert!(variant.bitrate > 0);
            assert!(!variant.content_type.is_empty());
            assert!(!variant.url.is_empty());
        }
    }
    if let Some(retweet) = &tweet.retweeted_tweet {
        assert!(tweet.raw_content.ends_with(&retweet.raw_content));
    }

    check_user(&tweet.user);
    let json = serde_json::to_string(tweet).unwrap();
    assert!(json.contains(&tweet.id_str));
}

#[test]
fn parses_search_payload() {
    let tweets = parse_tweets(&search_payload(), 20);
    assert_eq!(tweets.len(), 3);
    assert!(tweets.iter().map(|tweet| tweet.bookmarked_count).sum::<i64>() > 0);
    for tweet in &tweets {
        check_tweet(tweet);
    }
}

#[test]
fn parses_users_by_id_and_login_shape() {
    let user = parse_users(&user_payload(), -1).pop().unwrap();
    assert_eq!(user.id, 1001);
    assert_eq!(user.username, "xscraper_dev");
    check_user(&user);
}

#[test]
fn parses_tweet_details() {
    let tweet = parse_tweet(&tweet_payload(), 2001).unwrap();
    assert_eq!(tweet.id, 2001);
    check_tweet(&tweet);
}

#[test]
fn parses_trends() {
    let trends = parse_trends(&trend_payload(), -1);
    assert_eq!(trends.len(), 1);
    let trend = &trends[0];
    assert_eq!(trend.name, "XScraper");
    assert!(!trend.trend_url.url.is_empty());
    assert!(!trend.trend_url.url_type.is_empty());
    assert!(!trend.trend_url.url_endpoint_options.is_empty());
}

#[test]
fn parses_summary_card() {
    let tweet = parse_tweet(&tweet_payload(), 2001).unwrap();
    match tweet.card.unwrap() {
        Card::Summary { title, description, url, photo: Some(photo), .. } => {
            assert_eq!(title, "XScraper Card");
            assert_eq!(description, "Synthetic card");
            assert_eq!(url, "https://example.com/card");
            assert_eq!(photo.url, "https://example.com/card.jpg");
        }
        other => panic!("expected summary card photo, got {other:?}"),
    }
}

#[test]
fn parse_fixture_command_inputs_are_json_values() {
    let value: Value = search_payload();
    assert!(value.pointer("/data/search_by_raw_query").is_some());
}
