use crate::error::{Result, XScraperError};
use base64::Engine;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

const CLIENT_WEB_BASE: &str = "https://abs.twimg.com/responsive-web/client-web";

static INDICES_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(\(\w{1}\[(\d{1,2})\],\s*16\))+").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XclidAssetDiagnostics {
    pub ok: bool,
    #[serde(rename = "xclidScript")]
    pub xclid_script: Option<String>,
    #[serde(rename = "chunkIds")]
    pub chunk_ids: Vec<String>,
    pub markers: Vec<String>,
    pub failures: Vec<String>,
    #[serde(rename = "failureStage")]
    pub failure_stage: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XClientTransactionIdGenerator {
    vk_bytes: Vec<u8>,
    anim_key: String,
}

impl XClientTransactionIdGenerator {
    pub async fn create(client: &reqwest::Client) -> Result<Self> {
        let text = get_x_page_text(client, "https://x.com/tesla").await?;
        Self::from_html(client, &text).await
    }

    pub async fn from_html(client: &reqwest::Client, text: &str) -> Result<Self> {
        let anim_idx = parse_anim_idx(client, text).await?;
        let document = Html::parse_document(text);
        let vk_bytes = parse_vk_bytes(&document)?;
        let anim_arr = parse_anim_arr(&document, &vk_bytes)?;

        let mut frame_time = 1usize;
        for idx in anim_idx.iter().skip(1) {
            frame_time *= usize::from(vk_bytes[*idx]) % 16;
        }

        let frame_idx = usize::from(vk_bytes[anim_idx[0]]) % 16;
        let frame_row = anim_arr
            .get(frame_idx)
            .ok_or_else(|| XScraperError::LoginFlow("xclid frame row missing".into()))?;
        let frame_dur = frame_time as f64 / 4096.0;
        let anim_key = calc_anim_key(frame_row, frame_dur);

        Ok(Self { vk_bytes, anim_key })
    }

    pub fn from_parts(vk_bytes: Vec<u8>, anim_key: impl Into<String>) -> Self {
        Self { vk_bytes, anim_key: anim_key.into() }
    }

    pub fn calc(&self, method: &str, path: &str) -> String {
        let now_ms =
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
        let ts = ((now_ms - 1_682_924_400_000) / 1000) as u32;
        let ts_bytes = (0..4).map(|idx| ((ts >> (idx * 8)) & 0xff) as u8);

        let payload = format!(
            "{}!{}!{}{}{}",
            method.to_uppercase(),
            path,
            ts,
            "obfiowerehiring",
            self.anim_key
        );
        let digest = Sha256::digest(payload.as_bytes());
        let mut bytes = Vec::new();
        bytes.extend(&self.vk_bytes);
        bytes.extend(ts_bytes);
        bytes.extend(&digest[..16]);
        bytes.push(3);

        let random = rand::rng().random::<u8>();
        let mut obfuscated = Vec::with_capacity(bytes.len() + 1);
        obfuscated.push(random);
        obfuscated.extend(bytes.into_iter().map(|byte| byte ^ random));
        base64::engine::general_purpose::STANDARD
            .encode(obfuscated)
            .trim_end_matches('=')
            .to_string()
    }
}

async fn get_x_page_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let text = client.get(url).send().await?.error_for_status()?.text().await?;
    if !text.contains(">document.location =") {
        return Ok(text);
    }

    let next_url = text
        .split("document.location = \"")
        .nth(1)
        .and_then(|part| part.split('"').next())
        .ok_or_else(|| XScraperError::LoginFlow("x migration url missing".into()))?;
    let text = client.get(next_url).send().await?.error_for_status()?.text().await?;
    if !text.contains("action=\"https://x.com/x/migrate\" method=\"post\"") {
        return Ok(text);
    }

    let data = parse_migrate_inputs(&text);
    client
        .post("https://x.com/x/migrate")
        .json(&data)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .map_err(Into::into)
}

fn parse_migrate_inputs(text: &str) -> BTreeMap<String, String> {
    let mut data = BTreeMap::new();
    for part in text.split("<input").skip(1) {
        let Some(name) = part.split("name=\"").nth(1).and_then(|part| part.split('"').next())
        else {
            continue;
        };
        let Some(value) = part.split("value=\"").nth(1).and_then(|part| part.split('"').next())
        else {
            continue;
        };
        data.insert(name.to_string(), value.to_string());
    }
    data
}

fn get_scripts_list(text: &str) -> Result<Vec<String>> {
    let names = parse_chunk_map_after(text, "_.u=e=>\"\"+")?;
    let hashes = parse_chunk_map_before(text, ")[e]+\"a.js\"")?;
    let scripts = hashes
        .into_iter()
        .map(|(key, hash)| {
            let name = names.get(&key).cloned().unwrap_or(key);
            format!("{CLIENT_WEB_BASE}/{name}.{hash}a.js")
        })
        .collect::<Vec<_>>();
    if scripts.is_empty() {
        return Err(XScraperError::LoginFlow("xclid scripts marker missing".into()));
    }
    Ok(scripts)
}

pub fn diagnose_from_html(text: &str) -> XclidAssetDiagnostics {
    build_asset_diagnostics(text)
}

pub fn build_asset_diagnostics(text: &str) -> XclidAssetDiagnostics {
    let mut markers = Vec::new();
    let mut failures = Vec::new();

    if text.contains("_.u=e=>\"\"+") {
        markers.push("chunk-name-map".into());
    } else {
        failures.push("xclid chunk marker missing: webpack chunk name map marker not found".into());
    }
    if text.contains(")[e]+\"a.js\"") {
        markers.push("chunk-hash-map".into());
    } else {
        failures.push("xclid chunk marker missing: webpack chunk hash map marker not found".into());
    }

    let names = match parse_chunk_map_after(text, "_.u=e=>\"\"+") {
        Ok(values) => values,
        Err(error) => {
            failures.push(format!("chunk name map parse failed: {error}"));
            BTreeMap::new()
        }
    };
    let hashes = match parse_chunk_map_before(text, ")[e]+\"a.js\"") {
        Ok(values) => values,
        Err(error) => {
            failures.push(format!("chunk hash map parse failed: {error}"));
            BTreeMap::new()
        }
    };

    let chunk_ids = hashes.keys().cloned().collect::<Vec<_>>();
    let xclid_script = hashes.get("59924").map(|hash| {
        let name = names.get("59924").map(String::as_str).unwrap_or("59924");
        format!("{CLIENT_WEB_BASE}/{name}.{hash}a.js")
    });

    if xclid_script.is_none() && !hashes.is_empty() {
        failures.push("xclid ondemand chunk 59924 missing from chunk map".into());
    }

    let failure_stage = if xclid_script.is_none() { Some("chunk-map".into()) } else { None };
    let ok = failure_stage.is_none();

    XclidAssetDiagnostics { ok, xclid_script, chunk_ids, markers, failures, failure_stage }
}

fn parse_chunk_map_after(text: &str, start_marker: &str) -> Result<BTreeMap<String, String>> {
    let marker = text
        .find(start_marker)
        .ok_or_else(|| XScraperError::LoginFlow("xclid scripts marker missing".into()))?;
    let start = text[marker..]
        .find("({")
        .ok_or_else(|| XScraperError::LoginFlow("xclid scripts marker missing".into()))?
        + marker
        + 1;
    let end = text[start..]
        .find("})[e]")
        .ok_or_else(|| XScraperError::LoginFlow("xclid scripts marker missing".into()))?
        + start
        + 1;
    parse_quoted_numeric_map(&text[start..end])
}

fn parse_chunk_map_before(text: &str, end_marker: &str) -> Result<BTreeMap<String, String>> {
    let end = text
        .find(end_marker)
        .ok_or_else(|| XScraperError::LoginFlow("xclid scripts marker missing".into()))?;
    let start = text[..end]
        .rfind("({")
        .ok_or_else(|| XScraperError::LoginFlow("xclid scripts marker missing".into()))?
        + 1;
    parse_quoted_numeric_map(&text[start..end])
}

fn parse_quoted_numeric_map(raw: &str) -> Result<BTreeMap<String, String>> {
    let quoted = Regex::new(r#"(\d+):"([^"]+)""#)
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let values = quoted
        .captures_iter(raw)
        .filter_map(|captures| {
            let key = captures.get(1)?.as_str().to_string();
            let value = captures.get(2)?.as_str().to_string();
            Some((key, value))
        })
        .collect::<BTreeMap<_, _>>();
    if values.is_empty() {
        return Err(XScraperError::LoginFlow("xclid scripts marker missing".into()));
    }
    Ok(values)
}

async fn parse_anim_idx(client: &reqwest::Client, text: &str) -> Result<Vec<usize>> {
    let scripts = get_scripts_list(text)?
        .into_iter()
        .filter(|script| script.contains("/ondemand.s."))
        .collect::<Vec<_>>();
    let script = scripts
        .first()
        .ok_or_else(|| XScraperError::LoginFlow("xclid ondemand script missing".into()))?;
    let text = get_x_page_text(client, script).await?;
    let items = INDICES_REGEX
        .captures_iter(&text)
        .filter_map(|captures| captures.get(2)?.as_str().parse::<usize>().ok())
        .collect::<Vec<_>>();
    if items.is_empty() {
        return Err(XScraperError::LoginFlow("xclid indices missing".into()));
    }
    Ok(items)
}

fn parse_vk_bytes(document: &Html) -> Result<Vec<u8>> {
    let selector = Selector::parse(r#"meta[name="twitter-site-verification"][content]"#)
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let content = document
        .select(&selector)
        .next()
        .and_then(|element| element.value().attr("content"))
        .ok_or_else(|| XScraperError::LoginFlow("xclid verification key missing".into()))?;
    base64::engine::general_purpose::STANDARD
        .decode(content)
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))
}

fn parse_anim_arr(document: &Html, vk_bytes: &[u8]) -> Result<Vec<Vec<f64>>> {
    let selector = Selector::parse("svg[id^='loading-x-anim'] g:first-child path:nth-child(2)")
        .map_err(|error| XScraperError::LoginFlow(error.to_string()))?;
    let paths = document
        .select(&selector)
        .filter_map(|element| element.value().attr("d").map(str::trim))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err(XScraperError::LoginFlow("xclid animation array missing".into()));
    }
    let idx = usize::from(vk_bytes[5]) % paths.len();
    Ok(paths[idx][9..]
        .split('C')
        .map(|part| {
            part.chars()
                .map(|ch| if ch.is_ascii_digit() { ch } else { ' ' })
                .collect::<String>()
                .split_whitespace()
                .filter_map(|value| value.parse::<f64>().ok())
                .collect::<Vec<_>>()
        })
        .collect())
}

fn calc_anim_key(frames: &[f64], target_time: f64) -> String {
    let from_color = [frames[0], frames[1], frames[2], 1.0];
    let to_color = [frames[3], frames[4], frames[5], 1.0];
    let to_rotation = [solve(frames[6], 60.0, 360.0, true)];
    let curves = frames[7..]
        .iter()
        .enumerate()
        .map(|(idx, value)| solve(*value, if idx % 2 == 0 { 0.0 } else { -1.0 }, 1.0, false))
        .collect::<Vec<_>>();
    let value = Cubic::new(curves).get_value(target_time);
    let color = interpolate(&from_color, &to_color, value);
    let rotation = interpolate(&[0.0], &to_rotation, value);
    let matrix = get_rotation_matrix(rotation[0]);

    let mut parts = color[..3]
        .iter()
        .map(|value| format!("{:x}", value.max(0.0).round() as i64))
        .collect::<Vec<_>>();
    for value in matrix {
        let rounded = value.abs().round_to(2);
        let hex = float_to_hex(rounded);
        if hex.starts_with('.') {
            parts.push(format!("0{hex}").to_lowercase());
        } else if hex.is_empty() {
            parts.push("0".into());
        } else {
            parts.push(hex.to_lowercase());
        }
    }
    parts.extend(["0".into(), "0".into()]);
    parts.join("").replace(['.', '-'], "")
}

struct Cubic {
    curves: Vec<f64>,
}

impl Cubic {
    fn new(curves: Vec<f64>) -> Self {
        Self { curves }
    }

    fn get_value(&self, time: f64) -> f64 {
        let mut start = 0.0;
        let mut end = 1.0;
        let mut mid = 0.0;

        if time <= 0.0 {
            if self.curves[0] > 0.0 {
                return self.curves[1] / self.curves[0] * time;
            }
            if self.curves[1] == 0.0 && self.curves[2] > 0.0 {
                return self.curves[3] / self.curves[2] * time;
            }
            return 0.0;
        }

        if time >= 1.0 {
            if self.curves[2] < 1.0 {
                return 1.0 + (self.curves[3] - 1.0) / (self.curves[2] - 1.0) * (time - 1.0);
            }
            if self.curves[2] == 1.0 && self.curves[0] < 1.0 {
                return 1.0 + (self.curves[1] - 1.0) / (self.curves[0] - 1.0) * (time - 1.0);
            }
            return 1.0;
        }

        while start < end {
            mid = (start + end) / 2.0;
            let x_est = Self::calculate(self.curves[0], self.curves[2], mid);
            if (time - x_est).abs() < 0.00001 {
                return Self::calculate(self.curves[1], self.curves[3], mid);
            }
            if x_est < time {
                start = mid;
            } else {
                end = mid;
            }
        }
        Self::calculate(self.curves[1], self.curves[3], mid)
    }

    fn calculate(a: f64, b: f64, m: f64) -> f64 {
        3.0 * a * (1.0 - m) * (1.0 - m) * m + 3.0 * b * (1.0 - m) * m * m + m * m * m
    }
}

fn interpolate<const N: usize>(from: &[f64; N], to: &[f64; N], f: f64) -> [f64; N] {
    let mut out = [0.0; N];
    for idx in 0..N {
        out[idx] = from[idx] * (1.0 - f) + to[idx] * f;
    }
    out
}

fn get_rotation_matrix(rotation: f64) -> [f64; 4] {
    let rad = rotation.to_radians();
    [rad.cos(), -rad.sin(), rad.sin(), rad.cos()]
}

fn solve(value: f64, min: f64, max: f64, rounding: bool) -> f64 {
    let result = value * (max - min) / 255.0 + min;
    if rounding { result.floor() } else { result.round_to(2) }
}

fn float_to_hex(mut value: f64) -> String {
    let mut result = Vec::new();
    let mut quotient = value.trunc() as i64;
    let mut fraction = value - quotient as f64;

    while quotient > 0 {
        quotient = (value / 16.0) as i64;
        let remainder = (value - quotient as f64 * 16.0) as i64;
        result.insert(0, hex_digit(remainder));
        value = quotient as f64;
    }

    if fraction == 0.0 {
        return result.into_iter().collect();
    }

    result.push('.');
    let mut guard = 0;
    while fraction > 0.0 && guard < 12 {
        fraction *= 16.0;
        let integer = fraction.trunc() as i64;
        fraction -= integer as f64;
        result.push(hex_digit(integer));
        guard += 1;
    }
    result.into_iter().collect()
}

fn hex_digit(value: i64) -> char {
    if value > 9 {
        char::from_u32((value + 55) as u32).unwrap_or('0')
    } else {
        char::from_digit(value as u32, 10).unwrap_or('0')
    }
}

trait RoundTo {
    fn round_to(self, digits: u32) -> f64;
}

impl RoundTo for f64 {
    fn round_to(self, digits: u32) -> f64 {
        let factor = 10f64.powi(digits as i32);
        (self * factor).round() / factor
    }
}

#[cfg(test)]
mod tests {
    use super::{XClientTransactionIdGenerator, get_scripts_list};

    #[test]
    fn calc_returns_base64_like_transaction_id() {
        let generator = XClientTransactionIdGenerator::from_parts(vec![1, 2, 3, 4, 5, 6], "abcdef");
        let id = generator.calc("GET", "/i/api/graphql/test");
        assert!(id.len() > 20);
        assert!(!id.contains('='));
    }

    #[test]
    fn scripts_list_uses_current_webpack_chunk_names() {
        let html = r#"_.u=e=>""+(({59924:"ondemand.s",61093:"bundle.UserAbout"})[e]||e)+"."+({59924:"f7a413c",61093:"39d4cf7"})[e]+"a.js""#;

        let scripts = get_scripts_list(html).unwrap();

        assert!(scripts.contains(
            &"https://abs.twimg.com/responsive-web/client-web/ondemand.s.f7a413ca.js".to_string()
        ));
        assert!(
            scripts.contains(
                &"https://abs.twimg.com/responsive-web/client-web/bundle.UserAbout.39d4cf7a.js"
                    .to_string()
            )
        );
    }
}
