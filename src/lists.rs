use crate::error::{Result, XScraperError};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListTarget {
    Id(String),
    Slug { owner: String, slug: String },
}

impl ListTarget {
    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Id(id) => Some(id),
            Self::Slug { .. } => None,
        }
    }

    pub fn owner_and_slug(&self) -> Option<(&str, &str)> {
        match self {
            Self::Id(_) => None,
            Self::Slug { owner, slug } => Some((owner, slug)),
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Id(id) => id.clone(),
            Self::Slug { owner, slug } => format!("{owner}/lists/{slug}"),
        }
    }
}

pub fn normalize_list_target(input: &str) -> Result<ListTarget> {
    let trimmed = input.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(invalid_list_target(input));
    }

    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Ok(ListTarget::Id(trimmed.to_string()));
    }

    if let Ok(url) = Url::parse(trimmed)
        && is_x_host(url.host_str())
    {
        return list_target_from_url(&url).ok_or_else(|| invalid_list_target(input));
    }

    if let Some((owner, slug)) = trimmed.split_once("/lists/") {
        return list_slug_target(owner, slug).ok_or_else(|| invalid_list_target(input));
    }

    if let Some((owner, slug)) = trimmed.split_once('/') {
        return list_slug_target(owner, slug).ok_or_else(|| invalid_list_target(input));
    }

    Err(invalid_list_target(input))
}

pub fn normalize_list_id(input: &str) -> Result<String> {
    normalize_list_target(input)?.id().map(ToOwned::to_owned).ok_or_else(|| {
        XScraperError::Config(format!(
            "list id is required for this operation; got slug target {input}"
        ))
    })
}

fn list_target_from_url(url: &Url) -> Option<ListTarget> {
    let segments = url.path_segments()?.filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    match segments.as_slice() {
        ["i", "lists", id, ..] if id.chars().all(|ch| ch.is_ascii_digit()) => {
            Some(ListTarget::Id((*id).to_string()))
        }
        [owner, "lists", slug, ..] => list_slug_target(owner, slug),
        _ => None,
    }
}

fn list_slug_target(owner: &str, slug: &str) -> Option<ListTarget> {
    let owner = owner.trim().trim_start_matches('@');
    let slug = slug.trim().trim_end_matches('/');
    if owner.is_empty() || slug.is_empty() {
        return None;
    }
    Some(ListTarget::Slug { owner: owner.to_string(), slug: slug.to_string() })
}

fn is_x_host(host: Option<&str>) -> bool {
    matches!(
        host,
        Some("x.com")
            | Some("www.x.com")
            | Some("mobile.x.com")
            | Some("twitter.com")
            | Some("www.twitter.com")
            | Some("mobile.twitter.com")
    )
}

fn invalid_list_target(input: &str) -> XScraperError {
    XScraperError::Config(format!(
        "invalid list target {input}; expected a list id, https://x.com/i/lists/<id>, or owner/lists/<slug>"
    ))
}
