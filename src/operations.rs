use crate::gql::*;
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Item,
    Timeline,
}

#[derive(Debug, Clone)]
pub struct OperationRequest {
    pub op: &'static str,
    pub queue: &'static str,
    pub variables: Value,
    pub features: Option<Value>,
    pub field_toggles: Option<Value>,
    pub cursor_type: &'static str,
    pub mode: OperationMode,
}

pub fn operation_request(
    operation: &str,
    id_or_query: &str,
    kv: Option<Value>,
) -> Option<OperationRequest> {
    let operation = operation.replace('-', "_");
    let id = || id_or_query.parse::<u64>().ok();
    let request = match operation.as_str() {
        "search" => search_request(id_or_query, "Latest", "typed_query", kv),
        "search_user" => search_request(id_or_query, "People", "typed_query", kv),
        "search_trend" => search_request(id_or_query, "Latest", "trend_click", kv),
        "user_by_id" => OperationRequest {
            op: OP_USER_BY_REST_ID,
            queue: "UserByRestId",
            variables: merge_json(
                json!({
                    "userId": id()?.to_string(),
                    "withSafetyModeUserFields": true
                }),
                kv,
            ),
            features: Some(json!({
                "hidden_profile_likes_enabled": true,
                "highlights_tweets_tab_ui_enabled": true,
                "creator_subscriptions_tweet_preview_api_enabled": true,
                "hidden_profile_subscriptions_enabled": true,
                "responsive_web_twitter_article_notes_tab_enabled": false,
                "subscriptions_feature_can_gift_premium": false,
                "profile_label_improvements_pcf_label_in_post_enabled": false
            })),
            field_toggles: None,
            cursor_type: "",
            mode: OperationMode::Item,
        },
        "user_by_login" => OperationRequest {
            op: OP_USER_BY_SCREEN_NAME,
            queue: "UserByScreenName",
            variables: merge_json(
                json!({
                    "screen_name": id_or_query,
                    "withSafetyModeUserFields": true
                }),
                kv,
            ),
            features: Some(json!({
                "highlights_tweets_tab_ui_enabled": true,
                "hidden_profile_likes_enabled": true,
                "creator_subscriptions_tweet_preview_api_enabled": true,
                "hidden_profile_subscriptions_enabled": true,
                "subscriptions_verification_info_verified_since_enabled": true,
                "subscriptions_verification_info_is_identity_verified_enabled": false,
                "responsive_web_twitter_article_notes_tab_enabled": false,
                "subscriptions_feature_can_gift_premium": false,
                "profile_label_improvements_pcf_label_in_post_enabled": false
            })),
            field_toggles: None,
            cursor_type: "",
            mode: OperationMode::Item,
        },
        "tweet_details" => OperationRequest {
            op: OP_TWEET_DETAIL,
            queue: "TweetDetail",
            variables: tweet_detail_variables(id()?.to_string(), kv),
            features: None,
            field_toggles: None,
            cursor_type: "",
            mode: OperationMode::Item,
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
            field_toggles: None,
            cursor_type: "ShowMoreThreads",
            mode: OperationMode::Timeline,
        },
        "retweeters" => timeline_request(
            OP_RETWEETERS,
            json!({ "tweetId": id()?.to_string(), "count": 20, "includePromotedContent": true }),
            kv,
            None,
            None,
        ),
        "followers" => timeline_request(
            OP_FOLLOWERS,
            user_list_variables(id()?),
            kv,
            Some(json!({ "responsive_web_twitter_article_notes_tab_enabled": false })),
            None,
        ),
        "following" => timeline_request(OP_FOLLOWING, user_list_variables(id()?), kv, None, None),
        "verified_followers" => timeline_request(
            OP_BLUE_VERIFIED_FOLLOWERS,
            user_list_variables(id()?),
            kv,
            Some(json!({ "responsive_web_twitter_article_notes_tab_enabled": true })),
            None,
        ),
        "subscriptions" => timeline_request(
            OP_USER_CREATOR_SUBSCRIPTIONS,
            user_list_variables(id()?),
            kv,
            None,
            None,
        ),
        "user_tweets" => timeline_request(
            OP_USER_TWEETS,
            json!({
                "userId": id()?.to_string(),
                "count": 40,
                "includePromotedContent": true,
                "withQuickPromoteEligibilityTweetFields": true,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
            None,
            None,
        ),
        "user_tweets_and_replies" => timeline_request(
            OP_USER_TWEETS_AND_REPLIES,
            json!({
                "userId": id()?.to_string(),
                "count": 40,
                "includePromotedContent": true,
                "withCommunity": true,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
            None,
            None,
        ),
        "user_media" => timeline_request(
            OP_USER_MEDIA,
            json!({
                "userId": id()?.to_string(),
                "count": 40,
                "includePromotedContent": false,
                "withClientEventToken": false,
                "withBirdwatchNotes": false,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
            None,
            Some(json!({ "withArticlePlainText": false })),
        ),
        "list_timeline" => timeline_request(
            OP_LIST_LATEST_TWEETS_TIMELINE,
            json!({ "listId": id()?.to_string(), "count": 20 }),
            kv,
            None,
            Some(default_field_toggles()),
        ),
        "trends" => timeline_request(
            OP_GENERIC_TIMELINE_BY_ID,
            json!({
                "timelineId": trend_id(id_or_query),
                "count": 20,
                "withQuickPromoteEligibilityTweetFields": true
            }),
            kv,
            None,
            None,
        ),
        "bookmarks" => timeline_request(
            OP_BOOKMARKS,
            json!({
                "count": 20,
                "includePromotedContent": false,
                "withClientEventToken": false,
                "withBirdwatchNotes": false,
                "withVoice": true,
                "withV2Timeline": true
            }),
            kv,
            Some(json!({ "graphql_timeline_v2_bookmark_timeline": true })),
            None,
        ),
        _ => return None,
    };
    Some(request)
}

fn search_request(
    query: &str,
    product: &'static str,
    query_source: &'static str,
    kv: Option<Value>,
) -> OperationRequest {
    timeline_request(
        OP_SEARCH_TIMELINE,
        json!({
            "rawQuery": query,
            "count": 20,
            "product": product,
            "querySource": query_source,
            "withGrokTranslatedBio": false,
            "withQuickPromoteEligibilityTweetFields": false
        }),
        kv,
        None,
        Some(default_field_toggles()),
    )
}

fn timeline_request(
    op: &'static str,
    variables: Value,
    kv: Option<Value>,
    features: Option<Value>,
    field_toggles: Option<Value>,
) -> OperationRequest {
    OperationRequest {
        op,
        queue: op_name(op),
        variables: merge_json(variables, kv),
        features,
        field_toggles,
        cursor_type: "Bottom",
        mode: OperationMode::Timeline,
    }
}

fn tweet_detail_variables(tweet_id: String, kv: Option<Value>) -> Value {
    merge_json(
        json!({
            "focalTweetId": tweet_id,
            "with_rux_injections": true,
            "includePromotedContent": true,
            "withCommunity": true,
            "withQuickPromoteEligibilityTweetFields": true,
            "withBirdwatchNotes": true,
            "withVoice": true,
            "withV2Timeline": true
        }),
        kv,
    )
}

fn user_list_variables(user_id: u64) -> Value {
    json!({ "userId": user_id.to_string(), "count": 20, "includePromotedContent": false })
}

fn default_field_toggles() -> Value {
    json!({
        "withPayments": false,
        "withAuxiliaryUserLabels": false,
        "withArticleRichContentState": false,
        "withArticlePlainText": false,
        "withArticleSummaryText": false,
        "withArticleVoiceOver": false,
        "withGrokAnalyze": false,
        "withDisallowedReplyControls": false
    })
}
