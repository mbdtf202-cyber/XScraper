use serde_json::{Value, json};

pub const GQL_URL: &str = "https://x.com/i/api/graphql";

pub const OP_SEARCH_TIMELINE: &str = "AIdc203rPpK_k_2KWSdm7g/SearchTimeline";
pub const OP_USER_BY_REST_ID: &str = "WJ7rCtezBVT6nk6VM5R8Bw/UserByRestId";
pub const OP_USER_BY_SCREEN_NAME: &str = "1VOOyvKkiI3FMmkeDNxM9A/UserByScreenName";
pub const OP_TWEET_DETAIL: &str = "_8aYOgEDz35BrBcBal1-_w/TweetDetail";
pub const OP_FOLLOWERS: &str = "Elc_-qTARceHpztqhI9PQA/Followers";
pub const OP_FOLLOWING: &str = "C1qZ6bs-L3oc_TKSZyxkXQ/Following";
pub const OP_RETWEETERS: &str = "i-CI8t2pJD15euZJErEDrg/Retweeters";
pub const OP_USER_TWEETS: &str = "HeWHY26ItCfUmm1e6ITjeA/UserTweets";
pub const OP_USER_TWEETS_AND_REPLIES: &str = "OAx9yEcW3JA9bPo63pcYlA/UserTweetsAndReplies";
pub const OP_LIST_LATEST_TWEETS_TIMELINE: &str = "BkauSnPUDQTeeJsxq17opA/ListLatestTweetsTimeline";
pub const OP_BLUE_VERIFIED_FOLLOWERS: &str = "ZpmVpf_fBIUgdPErpq2wWg/BlueVerifiedFollowers";
pub const OP_USER_CREATOR_SUBSCRIPTIONS: &str = "7qcGrVKpcooih_VvJLA1ng/UserCreatorSubscriptions";
pub const OP_USER_MEDIA: &str = "vFPc2LVIu7so2uA_gHQAdg/UserMedia";
pub const OP_BOOKMARKS: &str = "-LGfdImKeQz0xS_jjUwzlA/Bookmarks";
pub const OP_GENERIC_TIMELINE_BY_ID: &str = "CT0YFEFf5GOYa5DJcxM91w/GenericTimelineById";

pub fn default_features() -> Value {
    json!({
        "articles_preview_enabled": false,
        "c9s_tweet_anatomy_moderator_badge_enabled": true,
        "communities_web_enable_tweet_community_results_fetch": true,
        "creator_subscriptions_quote_tweet_preview_enabled": false,
        "creator_subscriptions_tweet_preview_api_enabled": true,
        "freedom_of_speech_not_reach_fetch_enabled": true,
        "graphql_is_translatable_rweb_tweet_is_translatable_enabled": true,
        "longform_notetweets_consumption_enabled": true,
        "longform_notetweets_inline_media_enabled": true,
        "longform_notetweets_rich_text_read_enabled": true,
        "responsive_web_edit_tweet_api_enabled": true,
        "responsive_web_enhance_cards_enabled": false,
        "responsive_web_graphql_exclude_directive_enabled": true,
        "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
        "responsive_web_graphql_timeline_navigation_enabled": true,
        "responsive_web_media_download_video_enabled": false,
        "responsive_web_twitter_article_tweet_consumption_enabled": true,
        "rweb_tipjar_consumption_enabled": true,
        "rweb_video_timestamps_enabled": true,
        "standardized_nudges_misinfo": true,
        "tweet_awards_web_tipping_enabled": false,
        "tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled": true,
        "tweet_with_visibility_results_prefer_gql_media_interstitial_enabled": false,
        "tweetypie_unmention_optimization_enabled": true,
        "verified_phone_label_enabled": false,
        "view_counts_everywhere_api_enabled": true,
        "responsive_web_grok_analyze_button_fetch_trends_enabled": false,
        "premium_content_api_read_enabled": false,
        "profile_label_improvements_pcf_label_in_post_enabled": false,
        "responsive_web_grok_share_attachment_enabled": false,
        "responsive_web_grok_analyze_post_followups_enabled": false,
        "responsive_web_grok_image_annotation_enabled": false,
        "responsive_web_grok_analysis_button_from_backend": false,
        "responsive_web_jetfuel_frame": false,
        "rweb_video_screen_enabled": true,
        "responsive_web_grok_show_grok_translated_post": true
    })
}

pub fn merge_json(base: Value, overlay: Option<Value>) -> Value {
    let Some(overlay) = overlay else {
        return base;
    };

    match (base, overlay) {
        (Value::Object(mut left), Value::Object(right)) => {
            for (key, value) in right {
                left.insert(key, value);
            }
            Value::Object(left)
        }
        (_, right) => right,
    }
}

pub fn op_name(op: &str) -> &str {
    op.rsplit('/').next().unwrap_or(op)
}

pub fn trend_id(value: &str) -> String {
    match value {
        "trending" => "VGltZWxpbmU6DAC2CwABAAAACHRyZW5kaW5nAAA",
        "news" => "VGltZWxpbmU6DAC2CwABAAAABG5ld3MAAA",
        "sport" => "VGltZWxpbmU6DAC2CwABAAAABnNwb3J0cwAA",
        "entertainment" => "VGltZWxpbmU6DAC2CwABAAAADWVudGVydGFpbm1lbnQAAA",
        other => other,
    }
    .to_string()
}
