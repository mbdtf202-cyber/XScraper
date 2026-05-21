use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coordinates {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Place {
    pub id: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub country: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TextLink {
    pub url: String,
    pub text: Option<String>,
    pub tcourl: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserRef {
    pub id: u64,
    pub id_str: String,
    pub username: String,
    pub displayname: String,
    #[serde(rename = "_type")]
    pub object_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: u64,
    pub id_str: String,
    pub url: String,
    pub username: String,
    pub displayname: String,
    #[serde(rename = "rawDescription")]
    pub raw_description: String,
    pub created: DateTime<Utc>,
    #[serde(rename = "followersCount")]
    pub followers_count: i64,
    #[serde(rename = "friendsCount")]
    pub friends_count: i64,
    #[serde(rename = "statusesCount")]
    pub statuses_count: i64,
    #[serde(rename = "favouritesCount")]
    pub favourites_count: i64,
    #[serde(rename = "listedCount")]
    pub listed_count: i64,
    #[serde(rename = "mediaCount")]
    pub media_count: i64,
    pub location: String,
    #[serde(rename = "profileImageUrl")]
    pub profile_image_url: String,
    #[serde(rename = "profileBannerUrl")]
    pub profile_banner_url: Option<String>,
    pub protected: Option<bool>,
    pub verified: Option<bool>,
    pub blue: Option<bool>,
    #[serde(rename = "blueType")]
    pub blue_type: Option<String>,
    #[serde(rename = "descriptionLinks")]
    pub description_links: Vec<TextLink>,
    #[serde(rename = "pinnedIds")]
    pub pinned_ids: Vec<u64>,
    #[serde(rename = "_type")]
    pub object_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListInfo {
    pub id: u64,
    pub id_str: String,
    pub url: String,
    pub name: String,
    pub description: String,
    pub slug: Option<String>,
    pub mode: Option<String>,
    #[serde(rename = "memberCount")]
    pub member_count: i64,
    #[serde(rename = "subscriberCount")]
    pub subscriber_count: i64,
    pub following: Option<bool>,
    #[serde(rename = "isMember")]
    pub is_member: Option<bool>,
    pub muting: Option<bool>,
    pub pinning: Option<bool>,
    #[serde(rename = "bannerUrl")]
    pub banner_url: Option<String>,
    pub owner: Option<User>,
    #[serde(rename = "_type")]
    pub object_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tweet {
    pub id: u64,
    pub id_str: String,
    pub url: String,
    pub date: DateTime<Utc>,
    pub user: User,
    pub lang: String,
    #[serde(rename = "rawContent")]
    pub raw_content: String,
    #[serde(rename = "replyCount")]
    pub reply_count: i64,
    #[serde(rename = "retweetCount")]
    pub retweet_count: i64,
    #[serde(rename = "likeCount")]
    pub like_count: i64,
    #[serde(rename = "quoteCount")]
    pub quote_count: i64,
    #[serde(rename = "bookmarkedCount")]
    pub bookmarked_count: i64,
    #[serde(rename = "conversationId")]
    pub conversation_id: u64,
    #[serde(rename = "conversationIdStr")]
    pub conversation_id_str: String,
    pub hashtags: Vec<String>,
    pub cashtags: Vec<String>,
    #[serde(rename = "mentionedUsers")]
    pub mentioned_users: Vec<UserRef>,
    pub links: Vec<TextLink>,
    pub media: Media,
    #[serde(rename = "viewCount")]
    pub view_count: Option<i64>,
    #[serde(rename = "retweetedTweet")]
    pub retweeted_tweet: Option<Box<Tweet>>,
    #[serde(rename = "quotedTweet")]
    pub quoted_tweet: Option<Box<Tweet>>,
    pub place: Option<Place>,
    pub coordinates: Option<Coordinates>,
    #[serde(rename = "inReplyToTweetId")]
    pub in_reply_to_tweet_id: Option<u64>,
    #[serde(rename = "inReplyToTweetIdStr")]
    pub in_reply_to_tweet_id_str: Option<String>,
    #[serde(rename = "inReplyToUser")]
    pub in_reply_to_user: Option<UserRef>,
    pub source: Option<String>,
    #[serde(rename = "sourceUrl")]
    pub source_url: Option<String>,
    #[serde(rename = "sourceLabel")]
    pub source_label: Option<String>,
    pub card: Option<Card>,
    pub possibly_sensitive: Option<bool>,
    #[serde(rename = "_type")]
    pub object_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Media {
    pub photos: Vec<MediaPhoto>,
    pub videos: Vec<MediaVideo>,
    pub animated: Vec<MediaAnimated>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaPhoto {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaVideo {
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    pub variants: Vec<MediaVideoVariant>,
    pub duration: i64,
    pub views: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaVideoVariant {
    #[serde(rename = "contentType")]
    pub content_type: String,
    pub bitrate: i64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaAnimated {
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    #[serde(rename = "videoUrl")]
    pub video_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "_type")]
pub enum Card {
    #[serde(rename = "summary")]
    Summary {
        title: String,
        description: String,
        #[serde(rename = "vanityUrl")]
        vanity_url: String,
        url: String,
        photo: Option<MediaPhoto>,
        video: Option<MediaVideo>,
    },
    #[serde(rename = "poll")]
    Poll { options: Vec<PollOption>, finished: bool },
    #[serde(rename = "broadcast")]
    Broadcast { title: String, url: String, photo: Option<MediaPhoto> },
    #[serde(rename = "audiospace")]
    Audiospace { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PollOption {
    pub label: String,
    #[serde(rename = "votesCount")]
    pub votes_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestParam {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrendUrl {
    pub url: String,
    #[serde(rename = "urlType")]
    pub url_type: String,
    #[serde(rename = "urlEndpointOptions")]
    pub url_endpoint_options: Vec<RequestParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrendMetadata {
    pub domain_context: String,
    pub meta_description: String,
    pub url: TrendUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupedTrend {
    pub name: String,
    pub url: TrendUrl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trend {
    pub id: Option<String>,
    pub rank: Option<i64>,
    pub name: String,
    pub trend_url: TrendUrl,
    pub trend_metadata: TrendMetadata,
    pub grouped_trends: Vec<GroupedTrend>,
    #[serde(rename = "_type")]
    pub object_type: String,
}
