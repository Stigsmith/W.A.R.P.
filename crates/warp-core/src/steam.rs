//! Steam Web API: Workshop item details. Needs no API key and no game install.

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use serde_json::Value;

use crate::Error;
use crate::model::{ModInfo, WorkshopId};

/// Total War: WARHAMMER III.
pub const APP_ID: u32 = 1_142_710;

const DETAILS_URL: &str = "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";
const BATCH: usize = 100;

/// Fetches details for any number of items, in batches. Items Steam doesn't know
/// (deleted, private) come back with `available = false`.
pub fn fetch_details(ids: &[WorkshopId]) -> Result<Vec<ModInfo>, Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(30))).build().into();
    let mut out = Vec::with_capacity(ids.len());
    for chunk in ids.chunks(BATCH) {
        let mut form = vec![("itemcount".to_owned(), chunk.len().to_string())];
        form.extend(chunk.iter().enumerate().map(|(i, id)| (format!("publishedfileids[{i}]"), id.to_string())));
        let body: Value = with_retry(|| {
            agent
                .post(DETAILS_URL)
                .send_form(form.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                .and_then(|mut r| r.body_mut().read_json::<Value>())
        })?;
        out.extend(parse_details(&body)?);
    }
    Ok(out)
}

fn with_retry<T>(mut f: impl FnMut() -> Result<T, ureq::Error>) -> Result<T, Error> {
    let mut last = None;
    for attempt in 0..3 {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                last = Some(e);
                std::thread::sleep(Duration::from_millis(500 * (attempt + 1)));
            }
        }
    }
    Err(Error::Steam(format!("Steam didn't answer: {}", last.expect("attempted"))))
}

/// Parses a `GetPublishedFileDetails` response.
pub fn parse_details(body: &Value) -> Result<Vec<ModInfo>, Error> {
    let items = body["response"]["publishedfiledetails"]
        .as_array()
        .ok_or_else(|| Error::Steam("unexpected response from Steam".into()))?;
    Ok(items.iter().filter_map(parse_item).collect())
}

fn parse_item(v: &Value) -> Option<ModInfo> {
    let id = WorkshopId(as_u64(&v["publishedfileid"])?);
    let mut info = ModInfo::unknown(id);
    info.available = as_u64(&v["result"]) == Some(1);
    if !info.available {
        return Some(info);
    }
    info.title = v["title"].as_str().unwrap_or_default().trim().to_owned();
    info.description = clean_description(v["description"].as_str().unwrap_or_default());
    info.steam_tags = v["tags"]
        .as_array()
        .map(|tags| tags.iter().filter_map(|t| t["tag"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default();
    info.file_size = as_u64(&v["file_size"]).unwrap_or(0);
    info.time_created = as_u64(&v["time_created"]).unwrap_or(0) as i64;
    info.time_updated = as_u64(&v["time_updated"]).unwrap_or(0) as i64;
    info.preview_url = v["preview_url"].as_str().unwrap_or_default().to_owned();
    info.subscriptions = as_u64(&v["subscriptions"]).unwrap_or(0);
    info.favorited = as_u64(&v["favorited"]).unwrap_or(0);
    info.views = as_u64(&v["views"]).unwrap_or(0);
    Some(info)
}

/// Steam sends some numbers as strings.
fn as_u64(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str()?.parse().ok())
}

/// Workshop descriptions are BBCode with stray HTML; reduce them to plain text.
pub fn clean_description(text: &str) -> String {
    static BBCODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[/?[a-zA-Z*][^\]]*\]").expect("valid"));
    static HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("valid"));
    let text = BBCODE.replace_all(text, " ");
    let text = HTML.replace_all(&text, " ");
    let text = text
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_available_and_missing_items() {
        let body: Value = serde_json::from_str(
            r#"{"response":{"result":1,"resultcount":2,"publishedfiledetails":[
                {"publishedfileid":"2968554980","result":1,"title":" Submod hub ","description":"[h1]Hi[/h1] &amp; <b>bye</b>",
                 "file_size":"2128609","time_created":1675000000,"time_updated":1739000000,"preview_url":"https://x/y.png",
                 "subscriptions":10,"favorited":2,"views":300,"tags":[{"tag":"mod"},{"tag":"campaign"}]},
                {"publishedfileid":"123","result":9}
            ]}}"#,
        )
        .unwrap();
        let items = parse_details(&body).unwrap();
        assert_eq!(items.len(), 2);
        let a = &items[0];
        assert_eq!(a.id, WorkshopId(2_968_554_980));
        assert_eq!(a.title, "Submod hub");
        assert_eq!(a.description, "Hi & bye");
        assert_eq!(a.file_size, 2_128_609);
        assert_eq!(a.steam_tags, ["mod", "campaign"]);
        assert!(a.available);
        assert!(!items[1].available);
    }
}
