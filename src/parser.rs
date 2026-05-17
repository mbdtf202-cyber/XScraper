use crate::error::{Result, XScraperError};
use crate::models::*;
use crate::utils::{
    bool_path, collect_typed_objects, first_path, i64_path, parse_twitter_datetime, str_path,
    value_path,
};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Default)]
struct OldResponse {
    tweets: BTreeMap<String, Value>,
    users: BTreeMap<String, Value>,
    trends: BTreeMap<String, Value>,
}

pub fn parse_tweets(value: &Value, limit: i64) -> Vec<Tweet> {
    let response = to_old_response(value);
    let mut seen = HashSet::new();
    let mut tweets = Vec::new();

    for tweet in response.tweets.values() {
        if limit >= 0 && tweets.len() >= limit as usize {
            break;
        }

        match parse_tweet_from_old(tweet, &response) {
            Ok(parsed) if seen.insert(parsed.id) => tweets.push(parsed),
            Ok(_) => {}
            Err(error) => tracing::debug!("skipping tweet parse error: {error}"),
        }
    }

    tweets
}

pub fn parse_tweet(value: &Value, tweet_id: u64) -> Option<Tweet> {
    parse_tweets(value, -1).into_iter().find(|tweet| tweet.id == tweet_id)
}

pub fn parse_users(value: &Value, limit: i64) -> Vec<User> {
    let response = to_old_response(value);
    let mut seen = HashSet::new();
    let mut users = Vec::new();

    for user in response.users.values() {
        if limit >= 0 && users.len() >= limit as usize {
            break;
        }

        match parse_user_from_old(user) {
            Ok(parsed) if seen.insert(parsed.id) => users.push(parsed),
            Ok(_) => {}
            Err(error) => tracing::debug!("skipping user parse error: {error}"),
        }
    }

    users
}

pub fn parse_user(value: &Value) -> Option<User> {
    let users = parse_users(value, -1);
    (users.len() == 1).then(|| users.into_iter().next()).flatten()
}

pub fn parse_trends(value: &Value, limit: i64) -> Vec<Trend> {
    let response = to_old_response(value);
    let mut trends = Vec::new();

    for trend in response.trends.values() {
        if limit >= 0 && trends.len() >= limit as usize {
            break;
        }

        match parse_trend_from_old(trend) {
            Ok(parsed) => trends.push(parsed),
            Err(error) => tracing::debug!("skipping trend parse error: {error}"),
        }
    }

    trends
}

fn to_old_response(value: &Value) -> OldResponse {
    let mut response = OldResponse::default();

    let mut tweet_objects = Vec::new();
    collect_typed_objects(value, "Tweet", &mut tweet_objects);
    for object in tweet_objects {
        if object.contains_key("legacy")
            && let Some((id, old)) = to_old_object(object)
        {
            response.tweets.insert(id, old);
        }
    }

    let mut visibility_objects = Vec::new();
    collect_typed_objects(value, "TweetWithVisibilityResults", &mut visibility_objects);
    for object in visibility_objects {
        if let Some(tweet) = object.get("tweet").and_then(Value::as_object)
            && tweet.contains_key("legacy")
            && let Some((id, old)) = to_old_object(tweet)
        {
            response.tweets.insert(id, old);
        }
    }

    let mut user_objects = Vec::new();
    collect_typed_objects(value, "User", &mut user_objects);
    for object in user_objects {
        if object.contains_key("legacy")
            && let Some((id, old)) = to_old_object(object)
        {
            response.users.insert(id, old);
        }
    }

    let mut trend_objects = Vec::new();
    collect_typed_objects(value, "TimelineTrend", &mut trend_objects);
    for object in trend_objects {
        if let Some(name) = object.get("name").and_then(Value::as_str) {
            response.trends.insert(name.to_string(), Value::Object(object.clone()));
        }
    }

    response
}

fn to_old_object(object: &Map<String, Value>) -> Option<(String, Value)> {
    let rest_id = object.get("rest_id")?.as_str()?.to_string();
    let legacy = object.get("legacy")?.as_object()?;
    let mut merged = object.clone();
    for (key, value) in legacy {
        merged.insert(key.clone(), value.clone());
    }
    overlay_current_user_fields(object, &mut merged);
    merged.insert("id_str".into(), Value::String(rest_id.clone()));
    if let Ok(id) = rest_id.parse::<u64>() {
        merged.insert("id".into(), json!(id));
    }
    merged.insert("legacy".into(), Value::Null);
    Some((rest_id, Value::Object(merged)))
}

fn overlay_current_user_fields(source: &Map<String, Value>, merged: &mut Map<String, Value>) {
    for (source_path, target_key) in [
        ("core.screen_name", "screen_name"),
        ("core.name", "name"),
        ("core.created_at", "created_at"),
        ("avatar.image_url", "profile_image_url_https"),
        ("profile_bio.description", "description"),
        ("location.location", "location"),
        ("privacy.protected", "protected"),
        ("verification.verified", "verified"),
    ] {
        if !merged.contains_key(target_key)
            && let Some(value) = value_path(&Value::Object(source.clone()), source_path)
        {
            merged.insert(target_key.into(), value.clone());
        }
    }
}

fn parse_user_from_old(value: &Value) -> Result<User> {
    let id_str = required_str(value, "id_str")?;
    let username = required_str(value, "screen_name")?;
    Ok(User {
        id: id_str.parse().map_err(|message| XScraperError::Parse {
            path: "user.id_str".into(),
            message: format!("{message}"),
        })?,
        id_str: id_str.to_string(),
        url: format!("https://x.com/{username}"),
        username: username.to_string(),
        displayname: required_str(value, "name")?.to_string(),
        raw_description: str_path(value, "description").unwrap_or_default().to_string(),
        created: parse_twitter_datetime(required_str(value, "created_at")?)?,
        followers_count: i64_path(value, "followers_count").unwrap_or_default(),
        friends_count: i64_path(value, "friends_count").unwrap_or_default(),
        statuses_count: i64_path(value, "statuses_count").unwrap_or_default(),
        favourites_count: i64_path(value, "favourites_count").unwrap_or_default(),
        listed_count: i64_path(value, "listed_count").unwrap_or_default(),
        media_count: i64_path(value, "media_count").unwrap_or_default(),
        location: str_path(value, "location").unwrap_or_default().to_string(),
        profile_image_url: str_path(value, "profile_image_url_https")
            .unwrap_or_default()
            .to_string(),
        profile_banner_url: str_path(value, "profile_banner_url").map(ToOwned::to_owned),
        protected: bool_path(value, "protected"),
        verified: bool_path(value, "verified"),
        blue: bool_path(value, "is_blue_verified"),
        blue_type: str_path(value, "verified_type").map(ToOwned::to_owned),
        description_links: parse_links(value, &["entities.description.urls", "entities.url.urls"]),
        pinned_ids: value_path(value, "pinned_tweet_ids_str")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .filter_map(|raw| raw.parse::<u64>().ok())
                    .collect()
            })
            .unwrap_or_default(),
        object_type: "xscraper.User".into(),
    })
}

fn parse_tweet_from_old(value: &Value, response: &OldResponse) -> Result<Tweet> {
    let user_id = required_str(value, "user_id_str")?;
    let user_value = response.users.get(user_id).ok_or_else(|| XScraperError::Parse {
        path: format!("users.{user_id}"),
        message: "tweet user not found".into(),
    })?;
    let user = parse_user_from_old(user_value)?;
    let id_str = required_str(value, "id_str")?;
    let id = id_str.parse::<u64>().map_err(|message| XScraperError::Parse {
        path: "tweet.id_str".into(),
        message: format!("{message}"),
    })?;

    let retweeted_tweet = first_path(
        value,
        &[
            "retweeted_status_id_str",
            "retweeted_status_result.result.rest_id",
            "retweeted_status_result.result.tweet.rest_id",
        ],
    )
    .and_then(Value::as_str)
    .and_then(|id| response.tweets.get(id))
    .and_then(|tweet| parse_tweet_from_old(tweet, response).ok())
    .map(Box::new);

    let quoted_tweet = first_path(
        value,
        &[
            "quoted_status_id_str",
            "quoted_status_result.result.rest_id",
            "quoted_status_result.result.tweet.rest_id",
        ],
    )
    .and_then(Value::as_str)
    .and_then(|id| response.tweets.get(id))
    .and_then(|tweet| parse_tweet_from_old(tweet, response).ok())
    .map(Box::new);

    let mut raw_content = str_path(value, "note_tweet.note_tweet_results.result.text")
        .or_else(|| str_path(value, "full_text"))
        .unwrap_or_default()
        .to_string();

    if let Some(retweet) = &retweeted_tweet {
        let restored = format!("RT @{}: {}", retweet.user.username, retweet.raw_content);
        if raw_content.ends_with('…') && raw_content != restored {
            raw_content = restored;
        }
    }

    Ok(Tweet {
        id,
        id_str: id_str.to_string(),
        url: format!("https://x.com/{}/status/{id_str}", user.username),
        date: parse_twitter_datetime(required_str(value, "created_at")?)?,
        user,
        lang: str_path(value, "lang").unwrap_or_default().to_string(),
        raw_content,
        reply_count: i64_path(value, "reply_count").unwrap_or_default(),
        retweet_count: i64_path(value, "retweet_count").unwrap_or_default(),
        like_count: i64_path(value, "favorite_count").unwrap_or_default(),
        quote_count: i64_path(value, "quote_count").unwrap_or_default(),
        bookmarked_count: i64_path(value, "bookmark_count").unwrap_or_default(),
        conversation_id: str_path(value, "conversation_id_str")
            .unwrap_or(id_str)
            .parse()
            .unwrap_or(id),
        conversation_id_str: str_path(value, "conversation_id_str").unwrap_or(id_str).to_string(),
        hashtags: text_values(value, "entities.hashtags"),
        cashtags: text_values(value, "entities.symbols"),
        mentioned_users: parse_user_refs(value_path(value, "entities.user_mentions")),
        links: parse_links(
            value,
            &["entities.urls", "note_tweet.note_tweet_results.result.entity_set.urls"],
        ),
        media: parse_media(value),
        view_count: get_views(value, retweeted_tweet.as_deref()),
        retweeted_tweet,
        quoted_tweet,
        place: value_path(value, "place").and_then(parse_place),
        coordinates: parse_coordinates(value),
        in_reply_to_tweet_id: str_path(value, "in_reply_to_status_id_str")
            .and_then(|raw| raw.parse().ok()),
        in_reply_to_tweet_id_str: str_path(value, "in_reply_to_status_id_str")
            .map(ToOwned::to_owned),
        in_reply_to_user: parse_reply_user(value, response),
        source: str_path(value, "source").map(ToOwned::to_owned),
        source_url: parse_source_url(str_path(value, "source")),
        source_label: parse_source_label(str_path(value, "source")),
        card: parse_card(value),
        possibly_sensitive: bool_path(value, "possibly_sensitive"),
        object_type: "xscraper.Tweet".into(),
    })
}

fn parse_trend_from_old(value: &Value) -> Result<Trend> {
    let name = required_str(value, "name")?.to_string();
    Ok(Trend {
        id: Some(format!("trend-{name}")),
        rank: i64_path(value, "rank"),
        name,
        trend_url: parse_trend_url(required_value(value, "trend_url")?)?,
        trend_metadata: parse_trend_metadata(required_value(value, "trend_metadata")?)?,
        grouped_trends: value_path(value, "grouped_trends")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(|item| parse_grouped_trend(item).ok()).collect())
            .unwrap_or_default(),
        object_type: "timelinetrend".into(),
    })
}

fn parse_trend_url(value: &Value) -> Result<TrendUrl> {
    Ok(TrendUrl {
        url: required_str(value, "url")?.to_string(),
        url_type: required_str(value, "urlType")?.to_string(),
        url_endpoint_options: value_path(value, "urtEndpointOptions.requestParams")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(RequestParam {
                            key: str_path(item, "key")?.to_string(),
                            value: str_path(item, "value")?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn parse_trend_metadata(value: &Value) -> Result<TrendMetadata> {
    Ok(TrendMetadata {
        domain_context: required_str(value, "domain_context")?.to_string(),
        meta_description: required_str(value, "meta_description")?.to_string(),
        url: parse_trend_url(required_value(value, "url")?)?,
    })
}

fn parse_grouped_trend(value: &Value) -> Result<GroupedTrend> {
    Ok(GroupedTrend {
        name: required_str(value, "name")?.to_string(),
        url: parse_trend_url(required_value(value, "url")?)?,
    })
}

fn parse_links(value: &Value, paths: &[&str]) -> Vec<TextLink> {
    let mut links = Vec::new();
    for path in paths {
        if let Some(items) = value_path(value, path).and_then(Value::as_array) {
            for item in items {
                let Some(url) = str_path(item, "expanded_url") else {
                    continue;
                };
                let Some(tcourl) = str_path(item, "url") else {
                    continue;
                };
                links.push(TextLink {
                    url: url.to_string(),
                    text: str_path(item, "display_url").map(ToOwned::to_owned),
                    tcourl: Some(tcourl.to_string()),
                });
            }
        }
    }
    links
}

fn parse_user_refs(value: Option<&Value>) -> Vec<UserRef> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let id_str = str_path(item, "id_str")?;
                    Some(UserRef {
                        id: id_str.parse().ok()?,
                        id_str: id_str.to_string(),
                        username: str_path(item, "screen_name")?.to_string(),
                        displayname: str_path(item, "name")?.to_string(),
                        object_type: "xscraper.UserRef".into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_media(value: &Value) -> Media {
    let mut media = Media::default();
    let Some(items) = value_path(value, "extended_entities.media").and_then(Value::as_array) else {
        return media;
    };

    for item in items {
        match str_path(item, "type") {
            Some("photo") => {
                if let Some(url) = str_path(item, "media_url_https") {
                    media.photos.push(MediaPhoto { url: url.into() });
                }
            }
            Some("video") => {
                let variants = value_path(item, "video_info.variants")
                    .and_then(Value::as_array)
                    .map(|variants| {
                        variants
                            .iter()
                            .filter_map(|variant| {
                                Some(MediaVideoVariant {
                                    content_type: str_path(variant, "content_type")?.into(),
                                    bitrate: i64_path(variant, "bitrate")?,
                                    url: str_path(variant, "url")?.into(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if let (Some(thumbnail_url), Some(duration)) = (
                    str_path(item, "media_url_https"),
                    i64_path(item, "video_info.duration_millis"),
                ) {
                    media.videos.push(MediaVideo {
                        thumbnail_url: thumbnail_url.into(),
                        variants,
                        duration,
                        views: i64_path(item, "mediaStats.viewCount"),
                    });
                }
            }
            Some("animated_gif") => {
                if let (Some(thumbnail_url), Some(video_url)) = (
                    str_path(item, "media_url_https"),
                    value_path(item, "video_info.variants")
                        .and_then(Value::as_array)
                        .and_then(|items| items.first())
                        .and_then(|item| str_path(item, "url")),
                ) {
                    media.animated.push(MediaAnimated {
                        thumbnail_url: thumbnail_url.into(),
                        video_url: video_url.into(),
                    });
                }
            }
            _ => {}
        }
    }

    media
}

fn parse_place(value: &Value) -> Option<Place> {
    Some(Place {
        id: str_path(value, "id")?.into(),
        full_name: str_path(value, "full_name")?.into(),
        name: str_path(value, "name")?.into(),
        kind: str_path(value, "place_type")?.into(),
        country: str_path(value, "country")?.into(),
        country_code: str_path(value, "country_code")?.into(),
    })
}

fn parse_coordinates(value: &Value) -> Option<Coordinates> {
    if let Some(coords) = value_path(value, "coordinates.coordinates").and_then(Value::as_array) {
        return Some(Coordinates {
            longitude: coords.first()?.as_f64()?,
            latitude: coords.get(1)?.as_f64()?,
        });
    }

    if let Some(coords) = value_path(value, "geo.coordinates").and_then(Value::as_array) {
        return Some(Coordinates {
            longitude: coords.get(1)?.as_f64()?,
            latitude: coords.first()?.as_f64()?,
        });
    }

    None
}

fn parse_reply_user(value: &Value, response: &OldResponse) -> Option<UserRef> {
    let user_id = str_path(value, "in_reply_to_user_id_str")?;
    if let Some(user) = response.users.get(user_id) {
        return user_ref_from_user(user).ok();
    }

    parse_user_refs(value_path(value, "entities.user_mentions"))
        .into_iter()
        .find(|user| user.id_str == user_id)
}

fn user_ref_from_user(value: &Value) -> Result<UserRef> {
    let id_str = required_str(value, "id_str")?;
    Ok(UserRef {
        id: id_str.parse().map_err(|message| XScraperError::Parse {
            path: "user_ref.id_str".into(),
            message: format!("{message}"),
        })?,
        id_str: id_str.into(),
        username: required_str(value, "screen_name")?.into(),
        displayname: required_str(value, "name")?.into(),
        object_type: "xscraper.UserRef".into(),
    })
}

fn parse_source_url(source: Option<&str>) -> Option<String> {
    let source = source?;
    let href = source.split("href=\"").nth(1)?.split('"').next()?;
    Some(href.to_string())
}

fn parse_source_label(source: Option<&str>) -> Option<String> {
    let source = source?;
    Some(source.split('>').nth(1)?.split('<').next()?.to_string())
}

fn parse_card(value: &Value) -> Option<Card> {
    let name = str_path(value, "card.legacy.name")?;
    match name {
        "summary" | "summary_large_image" | "player" => {
            let values = card_values(value);
            Some(Card::Summary {
                title: card_title(&values),
                description: card_str(&values, "description").unwrap_or_default(),
                vanity_url: card_str(&values, "vanity_url").unwrap_or_default(),
                url: card_str(&values, "card_url").unwrap_or_default(),
                photo: largest_card_photo(&values),
                video: None,
            })
        }
        "unified_card" => parse_unified_card(value),
        n if n.starts_with("poll") && n.contains("choice_text_only") => {
            let values = card_values(value);
            let mut options = Vec::new();
            for idx in 1..=20 {
                let Some(label) = card_str(&values, &format!("choice{idx}_label")) else {
                    break;
                };
                let Some(count) = card_str(&values, &format!("choice{idx}_count")) else {
                    break;
                };
                options.push(PollOption { label, votes_count: count.parse().unwrap_or_default() });
            }
            Some(Card::Poll { options, finished: card_bool(&values, "counts_are_final") })
        }
        "745291183405076480:broadcast" => {
            let values = card_values(value);
            Some(Card::Broadcast {
                title: card_str(&values, "broadcast_title")?,
                url: card_str(&values, "broadcast_url")?,
                photo: largest_card_photo(&values),
            })
        }
        "3691233323:audiospace" => {
            Some(Card::Audiospace { url: card_str(&card_values(value), "card_url")? })
        }
        _ => None,
    }
}

fn parse_unified_card(value: &Value) -> Option<Card> {
    let values = card_values(value);
    let raw = card_str(&values, "unified_card")?;
    let value = serde_json::from_str::<Value>(&raw).ok()?;
    let media_entity = value_path(&value, "media_entities")
        .and_then(Value::as_object)
        .and_then(|object| object.values().next());

    let video =
        media_entity.filter(|media| str_path(media, "type") == Some("video")).and_then(|media| {
            parse_media(&json!({ "extended_entities": { "media": [media] } }))
                .videos
                .into_iter()
                .next()
        });
    let photo = media_entity
        .filter(|media| str_path(media, "type") == Some("photo"))
        .and_then(|media| str_path(media, "media_url_https"))
        .map(|url| MediaPhoto { url: url.into() });

    Some(Card::Summary {
        title: str_path(&value, "component_objects.details_1.data.title.content")
            .unwrap_or_default()
            .into(),
        description: str_path(&value, "component_objects.details_1.data.subtitle.content")
            .unwrap_or_default()
            .into(),
        vanity_url: str_path(
            &value,
            "destination_objects.browser_with_docked_media_1.data.url_data.vanity",
        )
        .unwrap_or_default()
        .into(),
        url: str_path(&value, "destination_objects.browser_with_docked_media_1.data.url_data.url")
            .unwrap_or_default()
            .into(),
        photo,
        video,
    })
}

fn card_values(value: &Value) -> Vec<&Value> {
    value_path(value, "card.legacy.binding_values")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| str_path(item, "value.type") != Some("IMAGE_COLOR"))
                .collect()
        })
        .unwrap_or_default()
}

fn card_str(values: &[&Value], key: &str) -> Option<String> {
    values
        .iter()
        .find(|value| str_path(value, "key") == Some(key))
        .and_then(|value| str_path(value, "value.string_value"))
        .map(ToOwned::to_owned)
}

fn card_bool(values: &[&Value], key: &str) -> bool {
    values
        .iter()
        .find(|value| str_path(value, "key") == Some(key))
        .and_then(|value| bool_path(value, "value.boolean_value"))
        .unwrap_or(false)
}

fn card_title(values: &[&Value]) -> String {
    values
        .iter()
        .filter_map(|value| {
            let key = str_path(value, "key")?;
            ((key == "title") || key.ends_with("_alt_text"))
                .then(|| str_path(value, "value.string_value"))
                .flatten()
        })
        .max_by_key(|title| title.len())
        .unwrap_or_default()
        .to_string()
}

fn largest_card_photo(values: &[&Value]) -> Option<MediaPhoto> {
    let mut best = None;
    let mut best_height = i64::MIN;
    for value in values.iter().filter(|value| str_path(value, "value.type") == Some("IMAGE")) {
        let height = i64_path(value, "value.image_value.height").unwrap_or_default();
        if height > best_height {
            best_height = height;
            best = Some(*value);
        }
    }
    best.and_then(|value| str_path(value, "value.image_value.url"))
        .map(|url| MediaPhoto { url: url.into() })
}

fn text_values(value: &Value, path: &str) -> Vec<String> {
    value_path(value, path)
        .and_then(Value::as_array)
        .map(|items| {
            items.iter().filter_map(|item| str_path(item, "text").map(ToOwned::to_owned)).collect()
        })
        .unwrap_or_default()
}

fn get_views(value: &Value, retweeted_tweet: Option<&Tweet>) -> Option<i64> {
    i64_path(value, "ext_views.count")
        .or_else(|| i64_path(value, "views.count"))
        .or_else(|| retweeted_tweet.and_then(|tweet| tweet.view_count))
}

fn required_value<'a>(value: &'a Value, path: &str) -> Result<&'a Value> {
    value_path(value, path)
        .ok_or_else(|| XScraperError::Parse { path: path.into(), message: "missing value".into() })
}

fn required_str<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    str_path(value, path)
        .ok_or_else(|| XScraperError::Parse { path: path.into(), message: "missing string".into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_search_payload() {
        let tweets = parse_tweets(&crate::parser::tests_support::search_payload(), 20);
        assert!(!tweets.is_empty());
        assert!(tweets.iter().map(|tweet| tweet.bookmarked_count).sum::<i64>() > 0);
        assert!(tweets.iter().all(|tweet| tweet.id.to_string() == tweet.id_str));
    }

    #[test]
    fn parses_user_payload() {
        let user = parse_user(&crate::parser::tests_support::user_payload()).unwrap();
        assert_eq!(user.id, 1001);
        assert_eq!(user.username, "xscraper_dev");
    }

    #[test]
    fn parses_tweet_details_payload() {
        let tweet = parse_tweet(&crate::parser::tests_support::tweet_payload(), 2001).unwrap();
        assert_eq!(tweet.id, 2001);
        assert_eq!(tweet.user.id, 1001);
    }

    #[test]
    fn parses_trends_payload() {
        let trends = parse_trends(&crate::parser::tests_support::trend_payload(), 20);
        assert!(!trends.is_empty());
        assert!(!trends[0].trend_url.url.is_empty());
        assert!(!trends[0].trend_metadata.meta_description.is_empty());
    }
}

#[cfg(test)]
mod tests_support {
    use serde_json::{Value, json};

    pub fn user_payload() -> Value {
        json!({"data": {"user": {"result": user_result("1001", "xscraper_dev", "XScraper Dev")}}})
    }

    pub fn tweet_payload() -> Value {
        json!({
            "data": {
                "threaded_conversation_with_injections_v2": {
                    "instructions": [{
                        "entries": [{
                            "content": {
                                "itemContent": {
                                    "tweet_results": {
                                        "result": tweet_result("2001", "Synthetic XScraper payload", "1001")
                                    }
                                }
                            }
                        }]
                    }]
                }
            }
        })
    }

    pub fn search_payload() -> Value {
        json!({
            "data": {
                "search_by_raw_query": {
                    "search_timeline": {
                        "timeline": {
                            "instructions": [{
                                "entries": [{
                                    "content": {
                                        "itemContent": {
                                            "tweet_results": {
                                                "result": tweet_result("2001", "Synthetic XScraper payload", "1001")
                                            }
                                        }
                                    }
                                }]
                            }]
                        }
                    }
                }
            }
        })
    }

    pub fn trend_payload() -> Value {
        json!({
            "data": {
                "viewer_v2": {
                    "user_results": {
                        "result": {
                            "timeline": {
                                "timeline": {
                                    "instructions": [{
                                        "entries": [{
                                            "content": {
                                                "__typename": "TimelineTimelineTrend",
                                                "trend": {
                                                    "__typename": "TimelineTrend",
                                                    "name": "XScraper",
                                                    "rank": 1,
                                                    "trend_url": trend_url(),
                                                    "trend_metadata": {
                                                        "domain_context": "Trending in Software",
                                                        "meta_description": "1,234 posts",
                                                        "url": trend_url()
                                                    },
                                                    "grouped_trends": []
                                                }
                                            }
                                        }]
                                    }]
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    fn trend_url() -> Value {
        json!({
            "url": "twitter://search/?query=XScraper",
            "urlType": "DeepLink",
            "urtEndpointOptions": {"requestParams": [{"key": "q", "value": "XScraper"}]}
        })
    }

    fn user_result(id: &str, username: &str, displayname: &str) -> Value {
        json!({
            "__typename": "User",
            "id": format!("VXNlcjo{id}"),
            "rest_id": id,
            "legacy": {
                "screen_name": username,
                "name": displayname,
                "description": "Synthetic account for XScraper tests",
                "created_at": "Mon Jan 02 03:04:05 +0000 2023",
                "followers_count": 42,
                "friends_count": 7,
                "statuses_count": 11,
                "favourites_count": 13,
                "listed_count": 2,
                "media_count": 3,
                "location": "Local",
                "profile_image_url_https": "https://example.com/avatar.jpg",
                "entities": {"description": {"urls": []}},
                "pinned_tweet_ids_str": ["2001"]
            }
        })
    }

    fn tweet_result(id: &str, text: &str, user_id: &str) -> Value {
        json!({
            "__typename": "Tweet",
            "rest_id": id,
            "legacy": {
                "created_at": "Tue Feb 07 08:09:10 +0000 2023",
                "user_id_str": user_id,
                "full_text": text,
                "lang": "en",
                "reply_count": 1,
                "retweet_count": 2,
                "favorite_count": 3,
                "quote_count": 4,
                "bookmark_count": 5,
                "conversation_id_str": id,
                "entities": {"hashtags": [], "symbols": [], "user_mentions": []}
            },
            "views": {"count": "99"},
            "core": {"user_results": {"result": user_result(user_id, "xscraper_dev", "XScraper Dev")}}
        })
    }
}
