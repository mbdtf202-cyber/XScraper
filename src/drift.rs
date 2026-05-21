use crate::error::{Result, XScraperError};
use crate::gql;
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

const CLIENT_WEB_BASE: &str = "https://abs.twimg.com/responsive-web/client-web/";
const X_HOME: &str = "https://x.com";

#[derive(Debug, Clone, Serialize)]
pub struct DriftReport {
    #[serde(rename = "mainJs")]
    pub main_js: Option<String>,
    #[serde(rename = "operationsChecked")]
    pub operations_checked: Vec<OperationDrift>,
    #[serde(rename = "searchFeatureFlags")]
    pub search_feature_flags: Vec<String>,
    #[serde(rename = "searchFieldToggles")]
    pub search_field_toggles: Vec<String>,
    #[serde(rename = "xclidScript")]
    pub xclid_script: Option<String>,
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationDrift {
    pub operation: String,
    pub local: String,
    pub remote: Option<String>,
    pub matches: bool,
}

pub async fn fetch_live_report() -> Result<DriftReport> {
    let client =
        reqwest::Client::builder().redirect(reqwest::redirect::Policy::limited(10)).build()?;
    let html = client.get(X_HOME).send().await?.error_for_status()?.text().await?;
    let main_js = extract_main_js_url(&html)?;
    let main = client.get(&main_js).send().await?.error_for_status()?.text().await?;
    let mut report = build_report(&html, &main);
    report.main_js = Some(main_js);
    Ok(report)
}

pub fn build_report(html: &str, main_js: &str) -> DriftReport {
    let operations = extract_operations(main_js);
    let operations_checked = local_operations()
        .into_iter()
        .map(|(operation, local)| {
            let remote = operations.get(operation).cloned();
            OperationDrift {
                operation: operation.into(),
                local: local.into(),
                matches: remote.as_deref() == Some(local),
                remote,
            }
        })
        .collect::<Vec<_>>();
    let (search_feature_flags, search_field_toggles) =
        extract_operation_metadata(main_js, "SearchTimeline").unwrap_or_default();
    let xclid_script = extract_xclid_script(html);
    let ok = operations_checked.iter().all(|item| item.matches) && xclid_script.is_some();
    DriftReport {
        main_js: None,
        operations_checked,
        search_feature_flags,
        search_field_toggles,
        xclid_script,
        ok,
    }
}

fn local_operations() -> Vec<(&'static str, &'static str)> {
    vec![
        ("SearchTimeline", gql::OP_SEARCH_TIMELINE),
        ("UserByRestId", gql::OP_USER_BY_REST_ID),
        ("UserByScreenName", gql::OP_USER_BY_SCREEN_NAME),
        ("TweetDetail", gql::OP_TWEET_DETAIL),
        ("Followers", gql::OP_FOLLOWERS),
        ("Following", gql::OP_FOLLOWING),
        ("UserTweets", gql::OP_USER_TWEETS),
        ("UserTweetsAndReplies", gql::OP_USER_TWEETS_AND_REPLIES),
        ("CombinedLists", gql::OP_COMBINED_LISTS),
        ("ListByRestId", gql::OP_LIST_BY_REST_ID),
        ("ListBySlug", gql::OP_LIST_BY_SLUG),
        ("ListLatestTweetsTimeline", gql::OP_LIST_LATEST_TWEETS_TIMELINE),
        ("ListMembers", gql::OP_LIST_MEMBERS),
        ("ListMemberships", gql::OP_LIST_MEMBERSHIPS),
        ("ListOwnerships", gql::OP_LIST_OWNERSHIPS),
        ("ListRankedTweetsTimeline", gql::OP_LIST_RANKED_TWEETS_TIMELINE),
        ("ListSubscribers", gql::OP_LIST_SUBSCRIBERS),
        ("BlueVerifiedFollowers", gql::OP_BLUE_VERIFIED_FOLLOWERS),
        ("UserCreatorSubscriptions", gql::OP_USER_CREATOR_SUBSCRIPTIONS),
        ("UserMedia", gql::OP_USER_MEDIA),
        ("GenericTimelineById", gql::OP_GENERIC_TIMELINE_BY_ID),
    ]
}

fn extract_main_js_url(html: &str) -> Result<String> {
    let re = Regex::new(r#"https://abs\.twimg\.com/responsive-web/client-web/main\.[^"]+?\.js"#)
        .map_err(|error| XScraperError::Config(error.to_string()))?;
    re.find(html)
        .map(|matched| matched.as_str().to_string())
        .ok_or_else(|| XScraperError::LoginFlow("drift main js missing".into()))
}

fn extract_operations(text: &str) -> BTreeMap<String, String> {
    let re = Regex::new(r#"\d+\(e\)\{e\.exports=\{queryId:"([^"]+)",operationName:"([^"]+)""#)
        .expect("operation regex compiles");
    re.captures_iter(text)
        .filter_map(|captures| {
            let query_id = captures.get(1)?.as_str();
            let operation = captures.get(2)?.as_str();
            Some((operation.to_string(), format!("{query_id}/{operation}")))
        })
        .collect()
}

fn extract_operation_metadata(text: &str, operation: &str) -> Option<(Vec<String>, Vec<String>)> {
    let needle = format!(r#"operationName:"{operation}""#);
    let idx = text.find(&needle)?;
    let metadata_start = text[idx..].find("metadata:{")? + idx;
    let start = text[metadata_start..].find("featureSwitches:[")?
        + metadata_start
        + "featureSwitches:[".len();
    let feature_end = text[start..].find("]")? + start;
    let toggle_start =
        text[feature_end..].find("fieldToggles:[")? + feature_end + "fieldToggles:[".len();
    let toggle_end = text[toggle_start..].find("]")? + toggle_start;
    Some((
        parse_string_array(&text[start..feature_end]),
        parse_string_array(&text[toggle_start..toggle_end]),
    ))
}

fn parse_string_array(raw: &str) -> Vec<String> {
    let re = Regex::new(r#""([^"]+)""#).expect("array regex compiles");
    re.captures_iter(raw)
        .filter_map(|captures| captures.get(1).map(|value| value.as_str().to_string()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn extract_xclid_script(html: &str) -> Option<String> {
    let names = parse_chunk_map_after(html, "_.u=e=>\"\"+").ok()?;
    let hashes = parse_chunk_map_before(html, ")[e]+\"a.js\"").ok()?;
    let hash = hashes.get("59924")?;
    let name = names.get("59924").map(String::as_str).unwrap_or("59924");
    Some(format!("{CLIENT_WEB_BASE}{name}.{hash}a.js"))
}

fn parse_chunk_map_after(text: &str, start_marker: &str) -> Result<BTreeMap<String, String>> {
    let marker = text
        .find(start_marker)
        .ok_or_else(|| XScraperError::LoginFlow("drift chunk marker missing".into()))?;
    let start = text[marker..]
        .find("({")
        .ok_or_else(|| XScraperError::LoginFlow("drift chunk marker missing".into()))?
        + marker
        + 1;
    let end = text[start..]
        .find("})[e]")
        .ok_or_else(|| XScraperError::LoginFlow("drift chunk marker missing".into()))?
        + start
        + 1;
    parse_quoted_numeric_map(&text[start..end])
}

fn parse_chunk_map_before(text: &str, end_marker: &str) -> Result<BTreeMap<String, String>> {
    let end = text
        .find(end_marker)
        .ok_or_else(|| XScraperError::LoginFlow("drift chunk marker missing".into()))?;
    let start = text[..end]
        .rfind("({")
        .ok_or_else(|| XScraperError::LoginFlow("drift chunk marker missing".into()))?
        + 1;
    parse_quoted_numeric_map(&text[start..end])
}

fn parse_quoted_numeric_map(raw: &str) -> Result<BTreeMap<String, String>> {
    let quoted = Regex::new(r#"(\d+):"([^"]+)""#)
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let values = quoted
        .captures_iter(raw)
        .filter_map(|captures| {
            Some((captures.get(1)?.as_str().to_string(), captures.get(2)?.as_str().to_string()))
        })
        .collect::<BTreeMap<_, _>>();
    if values.is_empty() {
        return Err(XScraperError::LoginFlow("drift chunk marker missing".into()));
    }
    Ok(values)
}
