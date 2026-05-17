use serde_json::{Value, json};

pub const OP_SEARCH_TIMELINE: &str = "Yw6L66Pw54NHKuq4Dp7b4Q/SearchTimeline";
pub const OP_USER_BY_REST_ID: &str = "VQfQ9wwYdk6j_u2O4vt64Q/UserByRestId";
pub const OP_USER_BY_SCREEN_NAME: &str = "IGgvgiOx4QZndDHuD3x9TQ/UserByScreenName";
pub const OP_TWEET_DETAIL: &str = "oCon7R-cgWRFy6EfZjaKfg/TweetDetail";
pub const OP_FOLLOWERS: &str = "_orfRBQae57vylFPH0Huhg/Followers";
pub const OP_FOLLOWING: &str = "F42cDX8PDFxkbjjq6JrM2w/Following";
pub const OP_RETWEETERS: &str = "i-CI8t2pJD15euZJErEDrg/Retweeters";
pub const OP_USER_TWEETS: &str = "36rb3Xj3iJ64Q-9wKDjCcQ/UserTweets";
pub const OP_USER_TWEETS_AND_REPLIES: &str = "D5eKzDa5ZoJuC1TCeAXbWA/UserTweetsAndReplies";
pub const OP_LIST_LATEST_TWEETS_TIMELINE: &str = "7UuJsFvnWuZo0HmxrzU42Q/ListLatestTweetsTimeline";
pub const OP_BLUE_VERIFIED_FOLLOWERS: &str = "crKOXrAHR3W3aPuKEJG8GA/BlueVerifiedFollowers";
pub const OP_USER_CREATOR_SUBSCRIPTIONS: &str = "-9O4xZ8ykY_Hf6kyHJX30A/UserCreatorSubscriptions";
pub const OP_USER_MEDIA: &str = "9EovraBTXJYGSEQXZqlLmQ/UserMedia";
pub const OP_BOOKMARKS: &str = "-LGfdImKeQz0xS_jjUwzlA/Bookmarks";
pub const OP_GENERIC_TIMELINE_BY_ID: &str = "_dGVIf1cY6xFanFNPsAzPQ/GenericTimelineById";

pub fn default_features() -> Value {
    json!({
        "articles_preview_enabled": true,
        "c9s_tweet_anatomy_moderator_badge_enabled": true,
        "communities_web_enable_tweet_community_results_fetch": true,
        "content_disclosure_ai_generated_indicator_enabled": true,
        "content_disclosure_indicator_enabled": true,
        "creator_subscriptions_tweet_preview_api_enabled": true,
        "freedom_of_speech_not_reach_fetch_enabled": true,
        "graphql_is_translatable_rweb_tweet_is_translatable_enabled": true,
        "longform_notetweets_consumption_enabled": true,
        "longform_notetweets_inline_media_enabled": false,
        "longform_notetweets_rich_text_read_enabled": true,
        "post_ctas_fetch_enabled": true,
        "premium_content_api_read_enabled": false,
        "profile_label_improvements_pcf_label_in_post_enabled": true,
        "responsive_web_edit_tweet_api_enabled": true,
        "responsive_web_enhance_cards_enabled": false,
        "responsive_web_graphql_skip_user_profile_image_extensions_enabled": false,
        "responsive_web_graphql_timeline_navigation_enabled": true,
        "responsive_web_twitter_article_tweet_consumption_enabled": true,
        "responsive_web_grok_analysis_button_from_backend": true,
        "responsive_web_grok_analyze_button_fetch_trends_enabled": false,
        "responsive_web_grok_analyze_post_followups_enabled": true,
        "responsive_web_grok_annotations_enabled": true,
        "responsive_web_grok_community_note_auto_translation_is_enabled": true,
        "responsive_web_grok_image_annotation_enabled": true,
        "responsive_web_grok_imagine_annotation_enabled": true,
        "responsive_web_grok_share_attachment_enabled": true,
        "responsive_web_grok_show_grok_translated_post": true,
        "responsive_web_jetfuel_frame": true,
        "responsive_web_profile_redirect_enabled": false,
        "rweb_cashtags_composer_attachment_enabled": true,
        "rweb_cashtags_enabled": true,
        "rweb_conversational_replies_downvote_enabled": false,
        "rweb_tipjar_consumption_enabled": false,
        "rweb_video_screen_enabled": false,
        "standardized_nudges_misinfo": true,
        "tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled": true,
        "verified_phone_label_enabled": false,
        "view_counts_everywhere_api_enabled": true
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
