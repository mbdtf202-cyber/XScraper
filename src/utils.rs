use crate::error::{Result, XScraperError};
use base64::Engine;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub fn parse_cookies(input: &str) -> Result<BTreeMap<String, String>> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(input)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .unwrap_or_else(|| input.to_string());

    if let Ok(value) = serde_json::from_str::<Value>(&decoded) {
        return parse_cookie_json(&value).ok_or_else(|| XScraperError::InvalidCookie(input.into()));
    }

    let mut cookies = BTreeMap::new();
    for part in decoded.split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }

        let Some((name, value)) = trimmed.split_once('=') else {
            return Err(XScraperError::InvalidCookie(input.into()));
        };
        cookies.insert(name.trim().to_string(), value.trim().to_string());
    }

    if cookies.is_empty() { Err(XScraperError::InvalidCookie(input.into())) } else { Ok(cookies) }
}

fn parse_cookie_json(value: &Value) -> Option<BTreeMap<String, String>> {
    match value {
        Value::Object(obj) if obj.contains_key("cookies") => parse_cookie_json(obj.get("cookies")?),
        Value::Object(obj) => map_string_values(obj),
        Value::Array(items) => {
            let mut cookies = BTreeMap::new();
            for item in items {
                let name = item.get("name")?.as_str()?;
                let value = item.get("value")?.as_str()?;
                cookies.insert(name.to_string(), value.to_string());
            }
            Some(cookies)
        }
        _ => None,
    }
}

fn map_string_values(obj: &Map<String, Value>) -> Option<BTreeMap<String, String>> {
    let mut cookies = BTreeMap::new();
    for (key, value) in obj {
        cookies.insert(key.clone(), value.as_str()?.to_string());
    }
    Some(cookies)
}

pub fn value_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;
    for part in path.split('.') {
        current = match current {
            Value::Object(obj) => obj.get(part)?,
            _ => return None,
        };
    }
    Some(current)
}

pub fn str_path<'a>(value: &'a Value, path: &str) -> Option<&'a str> {
    value_path(value, path)?.as_str()
}

pub fn i64_path(value: &Value, path: &str) -> Option<i64> {
    let value = value_path(value, path)?;
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        .or_else(|| value.as_str()?.parse::<i64>().ok())
}

pub fn u64_path(value: &Value, path: &str) -> Option<u64> {
    let value = value_path(value, path)?;
    value.as_u64().or_else(|| value.as_str()?.parse::<u64>().ok())
}

pub fn bool_path(value: &Value, path: &str) -> Option<bool> {
    value_path(value, path)?.as_bool()
}

pub fn first_path<'a>(value: &'a Value, paths: &[&str]) -> Option<&'a Value> {
    paths.iter().find_map(|path| value_path(value, path))
}

pub fn find_object<'a, F>(value: &'a Value, predicate: &F) -> Option<&'a Value>
where
    F: Fn(&Map<String, Value>) -> bool,
{
    match value {
        Value::Object(obj) => {
            if predicate(obj) {
                return Some(value);
            }
            obj.values().find_map(|child| find_object(child, predicate))
        }
        Value::Array(items) => items.iter().find_map(|child| find_object(child, predicate)),
        _ => None,
    }
}

pub fn collect_typed_objects<'a>(
    value: &'a Value,
    typename: &str,
    output: &mut Vec<&'a Map<String, Value>>,
) {
    match value {
        Value::Object(obj) => {
            if obj.get("__typename").and_then(Value::as_str).is_some_and(|kind| kind == typename) {
                output.push(obj);
            }

            for child in obj.values() {
                collect_typed_objects(child, typename, output);
            }
        }
        Value::Array(items) => {
            for child in items {
                collect_typed_objects(child, typename, output);
            }
        }
        _ => {}
    }
}

pub fn json_string_map(map: &BTreeMap<String, String>) -> Result<String> {
    serde_json::to_string(map).map_err(Into::into)
}

pub fn parse_json_string_map(raw: &str) -> Result<BTreeMap<String, String>> {
    if raw.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(raw).map_err(Into::into)
}

pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}

pub fn unix_ts() -> i64 {
    now_utc().timestamp()
}

pub fn parse_twitter_datetime(raw: &str) -> Result<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_str(raw, "%a %b %d %H:%M:%S %z %Y")
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(Into::into)
}

pub fn print_table(rows: &[BTreeMap<String, String>]) {
    if rows.is_empty() {
        return;
    }

    let headers: Vec<String> = rows[0].keys().cloned().collect();
    let widths: Vec<usize> = headers
        .iter()
        .map(|header| {
            rows.iter()
                .map(|row| row.get(header).map_or(0, String::len))
                .chain(std::iter::once(header.len()))
                .max()
                .unwrap_or(header.len())
        })
        .collect();

    for (idx, row) in std::iter::once(
        headers.iter().map(|header| (header.clone(), header.clone())).collect::<BTreeMap<_, _>>(),
    )
    .chain(rows.iter().cloned())
    .enumerate()
    {
        let line = headers
            .iter()
            .enumerate()
            .map(|(col, header)| {
                format!(
                    "{:<width$}",
                    row.get(header).cloned().unwrap_or_default(),
                    width = widths[col]
                )
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("{line}");
        if idx == 0 {
            println!(
                "{}",
                widths.iter().map(|width| "-".repeat(*width)).collect::<Vec<_>>().join("  ")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_cookies;

    #[test]
    fn parses_cookie_formats() {
        assert_eq!(parse_cookies("abc=123; def=456").unwrap()["abc"], "123");
        assert_eq!(parse_cookies(r#"{"abc":"123","def":"456"}"#).unwrap()["def"], "456");
        assert_eq!(parse_cookies(r#"[{"name":"abc","value":"123"}]"#).unwrap()["abc"], "123");
        assert_eq!(
            parse_cookies("eyJhYmMiOiAiMTIzIiwgImRlZiI6ICI0NTYifQ==").unwrap()["def"],
            "456"
        );
        assert!(parse_cookies("{invalid}").is_err());
    }
}
