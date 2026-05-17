use crate::error::Result;
use crate::gql::*;
use crate::models::{Trend, Tweet, User};
use crate::parser;
use crate::pool::AccountsPool;
use crate::queue_client::QueueClient;
use crate::utils::find_object;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct Api {
    pub pool: AccountsPool,
    config: ApiConfig,
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub proxy: Option<String>,
    pub base_url: String,
}

#[derive(Debug, Clone)]
pub struct OperationRequest {
    pub op: &'static str,
    pub queue: &'static str,
    pub variables: Value,
    pub features: Option<Value>,
    pub cursor_type: &'static str,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self { proxy: None, base_url: "https://x.com".into() }
    }
}

impl Api {
    pub fn new(pool: AccountsPool) -> Self {
        Self { pool, config: ApiConfig::default() }
    }

    pub fn with_config(pool: AccountsPool, config: ApiConfig) -> Self {
        Self { pool, config }
    }

    pub async fn search_raw(
        &self,
        query: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        let variables = merge_json(
            json!({
                "rawQuery": query,
                "count": 20,
                "product": "Latest",
                "querySource": "typed_query"
            }),
            kv,
        );
        self.gql_items(OP_SEARCH_TIMELINE, variables, None, limit, "Bottom").await
    }

    pub fn operation_request(
        &self,
        operation: &str,
        id_or_query: &str,
        limit_kv: Option<Value>,
    ) -> Option<OperationRequest> {
        operation_request(operation, id_or_query, limit_kv)
    }

    pub async fn search(&self, query: &str, limit: i64, kv: Option<Value>) -> Result<Vec<Tweet>> {
        let mut tweets = Vec::new();
        for page in self.search_raw(query, limit, kv).await? {
            tweets.extend(parser::parse_tweets(&page, limit));
            if limit >= 0 && tweets.len() >= limit as usize {
                tweets.truncate(limit as usize);
                break;
            }
        }
        Ok(tweets)
    }

    pub async fn search_user(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        let mut users = Vec::new();
        for page in self.search_raw(query, limit, Some(json!({ "product": "People" }))).await? {
            users.extend(parser::parse_users(&page, limit));
            if limit >= 0 && users.len() >= limit as usize {
                users.truncate(limit as usize);
                break;
            }
        }
        Ok(users)
    }

    pub async fn user_by_id_raw(&self, user_id: u64, kv: Option<Value>) -> Result<Option<Value>> {
        let variables = merge_json(
            json!({
                "userId": user_id.to_string(),
                "withSafetyModeUserFields": true
            }),
            kv,
        );
        let features = json!({
            "hidden_profile_likes_enabled": true,
            "highlights_tweets_tab_ui_enabled": true,
            "creator_subscriptions_tweet_preview_api_enabled": true,
            "hidden_profile_subscriptions_enabled": true,
            "responsive_web_twitter_article_notes_tab_enabled": false,
            "subscriptions_feature_can_gift_premium": false,
            "profile_label_improvements_pcf_label_in_post_enabled": false
        });
        self.gql_item(OP_USER_BY_REST_ID, variables, Some(features)).await
    }

    pub async fn user_by_id(&self, user_id: u64, kv: Option<Value>) -> Result<Option<User>> {
        Ok(self.user_by_id_raw(user_id, kv).await?.and_then(|value| parser::parse_user(&value)))
    }

    pub async fn user_by_login_raw(&self, login: &str, kv: Option<Value>) -> Result<Option<Value>> {
        let variables = merge_json(
            json!({
                "screen_name": login,
                "withSafetyModeUserFields": true
            }),
            kv,
        );
        let features = json!({
            "highlights_tweets_tab_ui_enabled": true,
            "hidden_profile_likes_enabled": true,
            "creator_subscriptions_tweet_preview_api_enabled": true,
            "hidden_profile_subscriptions_enabled": true,
            "subscriptions_verification_info_verified_since_enabled": true,
            "subscriptions_verification_info_is_identity_verified_enabled": false,
            "responsive_web_twitter_article_notes_tab_enabled": false,
            "subscriptions_feature_can_gift_premium": false,
            "profile_label_improvements_pcf_label_in_post_enabled": false
        });
        self.gql_item(OP_USER_BY_SCREEN_NAME, variables, Some(features)).await
    }

    pub async fn user_by_login(&self, login: &str, kv: Option<Value>) -> Result<Option<User>> {
        Ok(self.user_by_login_raw(login, kv).await?.and_then(|value| parser::parse_user(&value)))
    }

    pub async fn tweet_details_raw(
        &self,
        tweet_id: u64,
        kv: Option<Value>,
    ) -> Result<Option<Value>> {
        let variables = merge_json(
            json!({
                "focalTweetId": tweet_id.to_string(),
                "with_rux_injections": true,
                "includePromotedContent": true,
                "withCommunity": true,
                "withQuickPromoteEligibilityTweetFields": true,
                "withBirdwatchNotes": true,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
        );
        self.gql_item(OP_TWEET_DETAIL, variables, None).await
    }

    pub async fn tweet_details(&self, tweet_id: u64, kv: Option<Value>) -> Result<Option<Tweet>> {
        Ok(self
            .tweet_details_raw(tweet_id, kv)
            .await?
            .and_then(|value| parser::parse_tweet(&value, tweet_id)))
    }

    pub async fn tweet_replies_raw(
        &self,
        tweet_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        let variables = merge_json(
            json!({
                "focalTweetId": tweet_id.to_string(),
                "referrer": "tweet",
                "with_rux_injections": true,
                "includePromotedContent": true,
                "withCommunity": true,
                "withQuickPromoteEligibilityTweetFields": true,
                "withBirdwatchNotes": true,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
        );
        self.gql_items(OP_TWEET_DETAIL, variables, None, limit, "ShowMoreThreads").await
    }

    pub async fn tweet_replies(
        &self,
        tweet_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let mut tweets = Vec::new();
        for page in self.tweet_replies_raw(tweet_id, limit, kv).await? {
            tweets.extend(
                parser::parse_tweets(&page, limit)
                    .into_iter()
                    .filter(|tweet| tweet.in_reply_to_tweet_id == Some(tweet_id)),
            );
            if limit >= 0 && tweets.len() >= limit as usize {
                tweets.truncate(limit as usize);
                break;
            }
        }
        Ok(tweets)
    }

    pub async fn followers(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<User>> {
        let pages = self.followers_raw(user_id, limit, kv).await?;
        Ok(parse_users_from_pages(pages, limit))
    }

    pub async fn followers_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_FOLLOWERS,
            merge_json(
                json!({ "userId": user_id.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            Some(json!({ "responsive_web_twitter_article_notes_tab_enabled": false })),
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn following(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<User>> {
        let pages = self.following_raw(user_id, limit, kv).await?;
        Ok(parse_users_from_pages(pages, limit))
    }

    pub async fn following_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_FOLLOWING,
            merge_json(
                json!({ "userId": user_id.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn verified_followers(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<User>> {
        let pages = self.verified_followers_raw(user_id, limit, kv).await?;
        Ok(parse_users_from_pages(pages, limit))
    }

    pub async fn verified_followers_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_BLUE_VERIFIED_FOLLOWERS,
            merge_json(
                json!({ "userId": user_id.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            Some(json!({ "responsive_web_twitter_article_notes_tab_enabled": true })),
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn subscriptions(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<User>> {
        let pages = self.subscriptions_raw(user_id, limit, kv).await?;
        Ok(parse_users_from_pages(pages, limit))
    }

    pub async fn subscriptions_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_USER_CREATOR_SUBSCRIPTIONS,
            merge_json(
                json!({ "userId": user_id.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn retweeters(
        &self,
        tweet_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<User>> {
        let pages = self.retweeters_raw(tweet_id, limit, kv).await?;
        Ok(parse_users_from_pages(pages, limit))
    }

    pub async fn retweeters_raw(
        &self,
        tweet_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_RETWEETERS,
            merge_json(
                json!({ "tweetId": tweet_id.to_string(), "count": 20, "includePromotedContent": true }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn user_tweets(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let pages = self.user_tweets_raw(user_id, limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn user_tweets_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_USER_TWEETS,
            merge_json(
                json!({
                    "userId": user_id.to_string(),
                    "count": 40,
                    "includePromotedContent": true,
                    "withQuickPromoteEligibilityTweetFields": true,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn user_tweets_and_replies(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let pages = self.user_tweets_and_replies_raw(user_id, limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn user_tweets_and_replies_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_USER_TWEETS_AND_REPLIES,
            merge_json(
                json!({
                    "userId": user_id.to_string(),
                    "count": 40,
                    "includePromotedContent": true,
                    "withCommunity": true,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn user_media(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let tweets = parse_tweets_from_pages(self.user_media_raw(user_id, limit, kv).await?, limit);
        Ok(tweets
            .into_iter()
            .filter(|tweet| {
                !tweet.media.photos.is_empty()
                    || !tweet.media.videos.is_empty()
                    || !tweet.media.animated.is_empty()
            })
            .collect())
    }

    pub async fn user_media_raw(
        &self,
        user_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_USER_MEDIA,
            merge_json(
                json!({
                    "userId": user_id.to_string(),
                    "count": 40,
                    "includePromotedContent": false,
                    "withClientEventToken": false,
                    "withBirdwatchNotes": false,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn list_timeline(
        &self,
        list_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let pages = self.list_timeline_raw(list_id, limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn list_timeline_raw(
        &self,
        list_id: u64,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items(
            OP_LIST_LATEST_TWEETS_TIMELINE,
            merge_json(json!({ "listId": list_id.to_string(), "count": 20 }), kv),
            None,
            limit,
            "Bottom",
        )
        .await
    }

    pub async fn trends(&self, trend: &str, limit: i64, kv: Option<Value>) -> Result<Vec<Trend>> {
        let pages = self.trends_raw(trend, limit, kv).await?;
        let mut trends = Vec::new();
        for page in pages {
            trends.extend(parser::parse_trends(&page, limit));
            if limit >= 0 && trends.len() >= limit as usize {
                trends.truncate(limit as usize);
                break;
            }
        }
        Ok(trends)
    }

    pub async fn trends_raw(
        &self,
        trend: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        let variables = merge_json(
            json!({
                "timelineId": trend_id(trend),
                "count": 20,
                "withQuickPromoteEligibilityTweetFields": true
            }),
            kv,
        );
        self.gql_items(OP_GENERIC_TIMELINE_BY_ID, variables, None, limit, "Bottom").await
    }

    pub async fn search_trend(
        &self,
        query: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        self.search(query, limit, Some(merge_json(json!({ "querySource": "trend_click" }), kv)))
            .await
    }

    pub async fn bookmarks(&self, limit: i64, kv: Option<Value>) -> Result<Vec<Tweet>> {
        let pages = self.bookmarks_raw(limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn bookmarks_raw(&self, limit: i64, kv: Option<Value>) -> Result<Vec<Value>> {
        self.gql_items(
            OP_BOOKMARKS,
            merge_json(
                json!({
                    "count": 20,
                    "includePromotedContent": false,
                    "withClientEventToken": false,
                    "withBirdwatchNotes": false,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            Some(json!({ "graphql_timeline_v2_bookmark_timeline": true })),
            limit,
            "Bottom",
        )
        .await
    }

    async fn gql_item(
        &self,
        op: &str,
        variables: Value,
        features: Option<Value>,
    ) -> Result<Option<Value>> {
        let queue = op_name(op);
        let client = self.queue_client(queue);
        let mut session = client.open().await?;
        let params = gql_params(op, variables, features, None);
        let url = format!("{GQL_URL}/{op}");
        let response = session.get(&url, params).await?.map(|response| response.value);
        session.close().await?;
        Ok(response)
    }

    async fn gql_items(
        &self,
        op: &str,
        mut variables: Value,
        features: Option<Value>,
        limit: i64,
        cursor_type: &str,
    ) -> Result<Vec<Value>> {
        let queue = op_name(op);
        let client = self.queue_client(queue);
        let mut session = client.open().await?;
        let mut cursor: Option<String> = None;
        let mut total = 0usize;
        let mut pages = Vec::new();

        loop {
            if let Some(cursor) = cursor.as_ref() {
                variables["cursor"] = Value::String(cursor.clone());
            }

            let params = gql_params(op, variables.clone(), features.clone(), field_toggles(queue));
            let url = format!("{GQL_URL}/{op}");
            let Some(response) = session.get(&url, params).await? else {
                break;
            };

            let entries = response
                .value
                .pointer("/data")
                .and_then(|_| count_entries(&response.value))
                .unwrap_or_default();
            cursor = find_cursor(&response.value, cursor_type);
            total += entries;

            let has_items = entries > 0;
            pages.push(response.value);

            if !has_items || cursor.is_none() || (limit >= 0 && total >= limit as usize) {
                break;
            }
        }

        session.close().await?;
        Ok(pages)
    }

    fn queue_client(&self, queue: &str) -> QueueClient {
        QueueClient::new(self.pool.clone(), queue)
            .with_proxy(self.config.proxy.clone())
            .with_base_url(self.config.base_url.clone())
    }
}

fn gql_params(
    op: &str,
    variables: Value,
    features: Option<Value>,
    field_toggles: Option<Value>,
) -> Vec<(String, String)> {
    let mut params = vec![
        ("variables".into(), compact_json(&variables)),
        ("features".into(), compact_json(&merge_json(default_features(), features))),
    ];
    if let Some(field_toggles) = field_toggles {
        params.push(("fieldToggles".into(), compact_json(&field_toggles)));
    } else if matches!(op_name(op), "SearchTimeline" | "ListLatestTweetsTimeline") {
        params.push((
            "fieldToggles".into(),
            compact_json(&json!({ "withArticleRichContentState": false })),
        ));
    }
    params
}

fn field_toggles(queue: &str) -> Option<Value> {
    match queue {
        "SearchTimeline" | "ListLatestTweetsTimeline" => {
            Some(json!({ "withArticleRichContentState": false }))
        }
        "UserMedia" => Some(json!({ "withArticlePlainText": false })),
        _ => None,
    }
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".into())
}

fn find_cursor(value: &Value, cursor_type: &str) -> Option<String> {
    find_object(value, &|obj| {
        obj.get("cursorType").and_then(Value::as_str).is_some_and(|kind| kind == cursor_type)
    })
    .and_then(|value| value.get("value"))
    .and_then(Value::as_str)
    .map(ToOwned::to_owned)
}

fn count_entries(value: &Value) -> Option<usize> {
    find_object(value, &|obj| obj.contains_key("entries"))
        .and_then(|value| value.get("entries"))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter(|entry| {
                    entry.get("entryId").and_then(Value::as_str).is_none_or(|id| {
                        !id.starts_with("cursor-") && !id.starts_with("messageprompt-")
                    })
                })
                .count()
        })
}

fn parse_users_from_pages(pages: Vec<Value>, limit: i64) -> Vec<User> {
    let mut users = Vec::new();
    for page in pages {
        users.extend(parser::parse_users(&page, limit));
        if limit >= 0 && users.len() >= limit as usize {
            users.truncate(limit as usize);
            break;
        }
    }
    users
}

fn parse_tweets_from_pages(pages: Vec<Value>, limit: i64) -> Vec<Tweet> {
    let mut tweets = Vec::new();
    for page in pages {
        tweets.extend(parser::parse_tweets(&page, limit));
        if limit >= 0 && tweets.len() >= limit as usize {
            tweets.truncate(limit as usize);
            break;
        }
    }
    tweets
}

fn operation_request(
    operation: &str,
    id_or_query: &str,
    kv: Option<Value>,
) -> Option<OperationRequest> {
    let id = || id_or_query.parse::<u64>().ok();
    let request = match operation {
        "search" => OperationRequest {
            op: OP_SEARCH_TIMELINE,
            queue: "SearchTimeline",
            variables: merge_json(
                json!({
                    "rawQuery": id_or_query,
                    "count": 20,
                    "product": "Latest",
                    "querySource": "typed_query"
                }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        "tweet_replies" => OperationRequest {
            op: OP_TWEET_DETAIL,
            queue: "TweetDetail",
            variables: merge_json(
                json!({
                    "focalTweetId": id()?.to_string(),
                    "referrer": "tweet",
                    "with_rux_injections": true,
                    "includePromotedContent": true,
                    "withCommunity": true,
                    "withQuickPromoteEligibilityTweetFields": true,
                    "withBirdwatchNotes": true,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            features: None,
            cursor_type: "ShowMoreThreads",
        },
        "retweeters" => OperationRequest {
            op: OP_RETWEETERS,
            queue: "Retweeters",
            variables: merge_json(
                json!({ "tweetId": id()?.to_string(), "count": 20, "includePromotedContent": true }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        "followers" => OperationRequest {
            op: OP_FOLLOWERS,
            queue: "Followers",
            variables: merge_json(
                json!({ "userId": id()?.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            features: Some(json!({ "responsive_web_twitter_article_notes_tab_enabled": false })),
            cursor_type: "Bottom",
        },
        "following" => OperationRequest {
            op: OP_FOLLOWING,
            queue: "Following",
            variables: merge_json(
                json!({ "userId": id()?.to_string(), "count": 20, "includePromotedContent": false }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        "user_tweets" => OperationRequest {
            op: OP_USER_TWEETS,
            queue: "UserTweets",
            variables: merge_json(
                json!({
                    "userId": id()?.to_string(),
                    "count": 40,
                    "includePromotedContent": true,
                    "withQuickPromoteEligibilityTweetFields": true,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        "user_tweets_and_replies" => OperationRequest {
            op: OP_USER_TWEETS_AND_REPLIES,
            queue: "UserTweetsAndReplies",
            variables: merge_json(
                json!({
                    "userId": id()?.to_string(),
                    "count": 40,
                    "includePromotedContent": true,
                    "withCommunity": true,
                    "withVoice": true,
                    "withV2Timeline": true
                }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        "list_timeline" => OperationRequest {
            op: OP_LIST_LATEST_TWEETS_TIMELINE,
            queue: "ListLatestTweetsTimeline",
            variables: merge_json(json!({ "listId": id()?.to_string(), "count": 20 }), kv),
            features: None,
            cursor_type: "Bottom",
        },
        "trends" => OperationRequest {
            op: OP_GENERIC_TIMELINE_BY_ID,
            queue: "GenericTimelineById",
            variables: merge_json(
                json!({
                    "timelineId": trend_id(id_or_query),
                    "count": 20,
                    "withQuickPromoteEligibilityTweetFields": true
                }),
                kv,
            ),
            features: None,
            cursor_type: "Bottom",
        },
        _ => return None,
    };
    Some(request)
}
