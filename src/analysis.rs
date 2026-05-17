use crate::api::Api;
use crate::error::Result;
use crate::models::{Tweet, User};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct AccountAnalysisReport {
    pub login: String,
    pub user: Option<User>,
    pub window: AnalysisWindow,
    #[serde(rename = "fetchedCount")]
    pub fetched_count: usize,
    #[serde(rename = "tweetCount")]
    pub tweet_count: usize,
    pub originals: usize,
    pub replies: usize,
    pub retweets: usize,
    pub quotes: usize,
    #[serde(rename = "byDay")]
    pub by_day: BTreeMap<String, usize>,
    #[serde(rename = "engagementSum")]
    pub engagement_sum: i64,
    #[serde(rename = "avgEngagement")]
    pub avg_engagement: f64,
    #[serde(rename = "medianEngagement")]
    pub median_engagement: i64,
    #[serde(rename = "viewsSum")]
    pub views_sum: i64,
    #[serde(rename = "avgViews")]
    pub avg_views: f64,
    #[serde(rename = "topWords")]
    pub top_words: Vec<CountedTerm>,
    pub cashtags: Vec<CountedTerm>,
    pub hashtags: Vec<CountedTerm>,
    #[serde(rename = "linkDomains")]
    pub link_domains: Vec<CountedTerm>,
    #[serde(rename = "contractLikeCount")]
    pub contract_like_count: usize,
    pub latest: Vec<TweetExcerpt>,
    #[serde(rename = "topEngagement")]
    pub top_engagement: Vec<TweetExcerpt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountComparisonReport {
    pub window: AnalysisWindow,
    pub accounts: Vec<AccountAnalysisReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisWindow {
    pub days: i64,
    pub since: String,
    pub until: String,
    #[serde(rename = "timezone")]
    pub time_zone: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CountedTerm {
    pub term: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct TweetExcerpt {
    pub id: String,
    pub url: String,
    pub date: DateTime<Utc>,
    pub text: String,
    pub engagement: i64,
    pub views: Option<i64>,
    pub likes: i64,
    pub reposts: i64,
    pub replies: i64,
    pub quotes: i64,
}

pub async fn analyze_account(
    api: &Api,
    login_or_url: &str,
    days: i64,
    limit: i64,
) -> Result<AccountAnalysisReport> {
    let login = normalize_login(login_or_url);
    let window = analysis_window(days);
    let user = api.user_by_login(&login, None).await?;
    let query = format!("from:{login} since:{} until:{}", window.since, window.until);
    let tweets = api.search(&query, limit, None).await?;
    Ok(analyze_tweets(login, user, window, tweets))
}

pub async fn compare_accounts(
    api: &Api,
    logins: &[String],
    days: i64,
    limit: i64,
) -> Result<AccountComparisonReport> {
    let window = analysis_window(days);
    let mut accounts = Vec::new();
    for login in logins {
        accounts.push(analyze_account(api, login, days, limit).await?);
    }
    Ok(AccountComparisonReport { window, accounts })
}

pub fn analyze_tweets(
    login: String,
    user: Option<User>,
    window: AnalysisWindow,
    tweets: Vec<Tweet>,
) -> AccountAnalysisReport {
    let fetched_count = tweets.len();
    let tweets = dedupe_tweets(tweets);
    let mut by_day = BTreeMap::new();
    let mut words = HashMap::new();
    let mut cashtags = HashMap::new();
    let mut hashtags = HashMap::new();
    let mut link_domains = HashMap::new();
    let mut engagements = Vec::new();
    let mut views_sum = 0;
    let mut contract_like_count = 0usize;

    for tweet in &tweets {
        let day = tweet.date.format("%Y-%m-%d").to_string();
        *by_day.entry(day).or_insert(0) += 1;
        let engagement = engagement(tweet);
        engagements.push(engagement);
        if let Some(views) = tweet.view_count {
            views_sum += views;
        }
        contract_like_count += count_contract_like_tokens(&tweet.raw_content);
        for word in extract_words(&tweet.raw_content) {
            *words.entry(word).or_insert(0) += 1;
        }
        for cashtag in &tweet.cashtags {
            *cashtags.entry(format!("${}", cashtag.to_uppercase())).or_insert(0) += 1;
        }
        for hashtag in &tweet.hashtags {
            *hashtags.entry(format!("#{hashtag}")).or_insert(0) += 1;
        }
        for link in &tweet.links {
            if let Some(domain) = link_domain(&link.url) {
                *link_domains.entry(domain).or_insert(0) += 1;
            }
        }
    }

    engagements.sort_unstable();
    let engagement_sum = engagements.iter().sum::<i64>();
    let tweet_count = tweets.len();
    let mut latest = tweets.clone();
    latest.sort_by(|left, right| right.date.cmp(&left.date));
    let mut top_engagement = tweets.clone();
    top_engagement.sort_by_key(|tweet| std::cmp::Reverse(engagement(tweet)));

    AccountAnalysisReport {
        login,
        user,
        window,
        fetched_count,
        tweet_count,
        originals: tweets.iter().filter(|tweet| tweet.retweeted_tweet.is_none()).count(),
        replies: tweets.iter().filter(|tweet| tweet.in_reply_to_tweet_id.is_some()).count(),
        retweets: tweets.iter().filter(|tweet| tweet.retweeted_tweet.is_some()).count(),
        quotes: tweets.iter().filter(|tweet| tweet.quoted_tweet.is_some()).count(),
        by_day,
        engagement_sum,
        avg_engagement: average(engagement_sum, tweet_count),
        median_engagement: median(&engagements),
        views_sum,
        avg_views: average(views_sum, tweet_count),
        top_words: counted_terms(words, 25),
        cashtags: counted_terms(cashtags, 20),
        hashtags: counted_terms(hashtags, 20),
        link_domains: counted_terms(link_domains, 10),
        contract_like_count,
        latest: latest.iter().take(10).map(excerpt).collect(),
        top_engagement: top_engagement.iter().take(10).map(excerpt).collect(),
    }
}

pub fn normalize_login(input: &str) -> String {
    let trimmed = input.trim().trim_end_matches('/');
    let value = if let Some(idx) = trimmed.rfind("x.com/") {
        &trimmed[idx + "x.com/".len()..]
    } else if let Some(idx) = trimmed.rfind("twitter.com/") {
        &trimmed[idx + "twitter.com/".len()..]
    } else {
        trimmed
    };
    value.trim_start_matches('@').split('/').next().unwrap_or(value).to_string()
}

fn analysis_window(days: i64) -> AnalysisWindow {
    let days = days.max(1);
    let until = Utc::now().date_naive() + Duration::days(1);
    let since = until - Duration::days(days);
    AnalysisWindow {
        days,
        since: format_date(since),
        until: format_date(until),
        time_zone: "UTC".into(),
    }
}

fn format_date(date: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

fn dedupe_tweets(tweets: Vec<Tweet>) -> Vec<Tweet> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for tweet in tweets {
        if seen.insert(tweet.id) {
            deduped.push(tweet);
        }
    }
    deduped
}

fn engagement(tweet: &Tweet) -> i64 {
    tweet.like_count + tweet.retweet_count + tweet.reply_count + tweet.quote_count
}

fn average(sum: i64, count: usize) -> f64 {
    if count == 0 { 0.0 } else { ((sum as f64 / count as f64) * 100.0).round() / 100.0 }
}

fn median(sorted_values: &[i64]) -> i64 {
    sorted_values.get(sorted_values.len() / 2).copied().unwrap_or_default()
}

fn counted_terms(values: HashMap<String, usize>, limit: usize) -> Vec<CountedTerm> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    values.into_iter().take(limit).map(|(term, count)| CountedTerm { term, count }).collect()
}

fn extract_words(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(str::trim)
        .filter(|word| word.len() >= 3)
        .map(|word| word.to_ascii_uppercase())
        .filter(|word| !STOP_WORDS.contains(&word.as_str()))
        .collect()
}

fn link_domain(raw: &str) -> Option<String> {
    Url::parse(raw).ok()?.host_str().map(|host| host.trim_start_matches("www.").to_string())
}

fn count_contract_like_tokens(text: &str) -> usize {
    text.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
        .filter(|token| {
            let token = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
            token.starts_with("0x") && token.len() == 42
                || (token.len() >= 32
                    && token.len() <= 44
                    && token.chars().all(|ch| ch.is_ascii_alphanumeric()))
        })
        .count()
}

fn excerpt(tweet: &Tweet) -> TweetExcerpt {
    TweetExcerpt {
        id: tweet.id_str.clone(),
        url: tweet.url.clone(),
        date: tweet.date,
        text: compact_text(&tweet.raw_content, 360),
        engagement: engagement(tweet),
        views: tweet.view_count,
        likes: tweet.like_count,
        reposts: tweet.retweet_count,
        replies: tweet.reply_count,
        quotes: tweet.quote_count,
    }
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let mut value = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.chars().count() > max_chars {
        value = value.chars().take(max_chars).collect::<String>();
    }
    value
}

const STOP_WORDS: &[&str] = &[
    "THE", "AND", "FOR", "ARE", "YOU", "THIS", "THAT", "WITH", "FROM", "HAVE", "HAS", "WAS",
    "WERE", "WILL", "JUST", "THEY", "THEM", "YOUR", "ABOUT", "INTO", "OVER", "WHEN", "WHAT",
    "WHERE", "HOW", "WHY", "ALL", "CAN", "NOT", "BUT", "ONE", "OUT", "OUR", "MORE", "ITS", "GET",
    "NEW", "NOW", "VIA", "ETH", "BTC", "SOL", "HTTPS", "HTTP", "COM", "STATUS",
];
