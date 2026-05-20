use crate::error::{Result, XScraperError};
use crate::gql::{default_features, merge_json};
use crate::models::{Trend, Tweet, User};
pub use crate::operations::OperationRequest;
use crate::operations::{OperationMode, operation_request};
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
        self.gql_items_request(self.request("search", query, kv)?, limit).await
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
        for page in self.search_user_raw(query, limit, None).await? {
            users.extend(parser::parse_users(&page, limit));
            if limit >= 0 && users.len() >= limit as usize {
                users.truncate(limit as usize);
                break;
            }
        }
        Ok(users)
    }

    pub async fn search_user_raw(
        &self,
        query: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items_request(self.request("search_user", query, kv)?, limit).await
    }

    pub async fn user_by_id_raw(&self, user_id: u64, kv: Option<Value>) -> Result<Option<Value>> {
        self.gql_item_request(self.request("user_by_id", &user_id.to_string(), kv)?).await
    }

    pub async fn user_by_id(&self, user_id: u64, kv: Option<Value>) -> Result<Option<User>> {
        Ok(self.user_by_id_raw(user_id, kv).await?.and_then(|value| parser::parse_user(&value)))
    }

    pub async fn user_by_login_raw(&self, login: &str, kv: Option<Value>) -> Result<Option<Value>> {
        self.gql_item_request(self.request("user_by_login", login, kv)?).await
    }

    pub async fn user_by_login(&self, login: &str, kv: Option<Value>) -> Result<Option<User>> {
        Ok(self.user_by_login_raw(login, kv).await?.and_then(|value| parser::parse_user(&value)))
    }

    pub async fn tweet_details_raw(
        &self,
        tweet_id: u64,
        kv: Option<Value>,
    ) -> Result<Option<Value>> {
        self.gql_item_request(self.request("tweet_details", &tweet_id.to_string(), kv)?).await
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
        self.gql_items_request(self.request("tweet_replies", &tweet_id.to_string(), kv)?, limit)
            .await
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
        self.gql_items_request(self.request("followers", &user_id.to_string(), kv)?, limit).await
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
        self.gql_items_request(self.request("following", &user_id.to_string(), kv)?, limit).await
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
        self.gql_items_request(self.request("verified_followers", &user_id.to_string(), kv)?, limit)
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
        self.gql_items_request(self.request("subscriptions", &user_id.to_string(), kv)?, limit)
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
        self.gql_items_request(self.request("retweeters", &tweet_id.to_string(), kv)?, limit).await
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
        self.gql_items_request(self.request("user_tweets", &user_id.to_string(), kv)?, limit).await
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
        self.gql_items_request(
            self.request("user_tweets_and_replies", &user_id.to_string(), kv)?,
            limit,
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
        self.gql_items_request(self.request("user_media", &user_id.to_string(), kv)?, limit).await
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
        self.gql_items_request(self.request("list_timeline", &list_id.to_string(), kv)?, limit)
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
        self.gql_items_request(self.request("trends", trend, kv)?, limit).await
    }

    pub async fn search_trend(
        &self,
        query: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Tweet>> {
        let pages = self.search_trend_raw(query, limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn search_trend_raw(
        &self,
        query: &str,
        limit: i64,
        kv: Option<Value>,
    ) -> Result<Vec<Value>> {
        self.gql_items_request(self.request("search_trend", query, kv)?, limit).await
    }

    pub async fn bookmarks(&self, limit: i64, kv: Option<Value>) -> Result<Vec<Tweet>> {
        let pages = self.bookmarks_raw(limit, kv).await?;
        Ok(parse_tweets_from_pages(pages, limit))
    }

    pub async fn bookmarks_raw(&self, limit: i64, kv: Option<Value>) -> Result<Vec<Value>> {
        self.gql_items_request(self.request("bookmarks", "", kv)?, limit).await
    }

    fn request(
        &self,
        operation: &str,
        id_or_query: &str,
        kv: Option<Value>,
    ) -> Result<OperationRequest> {
        operation_request(operation, id_or_query, kv).ok_or_else(|| {
            XScraperError::Config(format!("unsupported GraphQL operation request: {operation}"))
        })
    }

    async fn gql_item_request(&self, request: OperationRequest) -> Result<Option<Value>> {
        if request.mode != OperationMode::Item {
            return Err(XScraperError::Config(format!(
                "operation {} is not an item request",
                request.queue
            )));
        }

        let client = self.queue_client(request.queue);
        let mut session = client.open().await?;
        let body = gql_body(request.variables, request.features, request.field_toggles);
        let result = session
            .post_json(&gql_path(request.op), body)
            .await
            .map(|response| response.map(|response| response.value));
        let close_result = session.close().await;
        match (result, close_result) {
            (Ok(response), Ok(())) => Ok(response),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    async fn gql_items_request(&self, request: OperationRequest, limit: i64) -> Result<Vec<Value>> {
        if request.mode != OperationMode::Timeline {
            return Err(XScraperError::Config(format!(
                "operation {} is not a timeline request",
                request.queue
            )));
        }

        let client = self.queue_client(request.queue);
        let mut session = client.open().await?;
        let result: Result<Vec<Value>> = async {
            let mut variables = request.variables;
            let features = request.features;
            let field_toggles = request.field_toggles;
            let mut cursor: Option<String> = None;
            let mut total = 0usize;
            let mut pages = Vec::new();

            loop {
                if let Some(cursor) = cursor.as_ref() {
                    variables["cursor"] = Value::String(cursor.clone());
                }

                let body = gql_body(variables.clone(), features.clone(), field_toggles.clone());
                let Some(response) = session.post_json(&gql_path(request.op), body).await? else {
                    break;
                };

                let entries = response
                    .value
                    .pointer("/data")
                    .and_then(|_| count_entries(&response.value))
                    .unwrap_or_default();
                cursor = find_cursor(&response.value, request.cursor_type);
                total += entries;

                let has_items = entries > 0;
                pages.push(response.value);

                if !has_items || cursor.is_none() || (limit >= 0 && total >= limit as usize) {
                    break;
                }
            }

            Ok(pages)
        }
        .await;

        let close_result = session.close().await;
        match (result, close_result) {
            (Ok(pages), Ok(())) => Ok(pages),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    fn queue_client(&self, queue: &str) -> QueueClient {
        QueueClient::new(self.pool.clone(), queue)
            .with_proxy(self.config.proxy.clone())
            .with_base_url(self.config.base_url.clone())
    }
}

fn gql_body(variables: Value, features: Option<Value>, field_toggles: Option<Value>) -> Value {
    let mut body = json!({
        "variables": variables,
        "features": merge_json(default_features(), features),
    });
    if let Some(field_toggles) = field_toggles {
        body["fieldToggles"] = field_toggles;
    }
    body
}

fn gql_path(op: &str) -> String {
    format!("/i/api/graphql/{op}")
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
