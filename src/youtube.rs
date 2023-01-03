use std::{env, time::Duration};
use regex::Regex;
use serde_json::Value;

fn get_api_key() -> Result<String, String> {
    return env::var("YOUTUBE_API_KEY").map_err(stringify_error);
}

fn stringify_error(e: impl std::fmt::Debug + std::fmt::Display) -> String {
    format!("{}", e)
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeSearchResult {
    pub kind: String,
    pub etag: String,
    pub next_page_token: String,
    pub region_code: String,
    pub page_info: PageInfo,
    pub items: Vec<Item>,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    pub total_results: i64,
    pub results_per_page: i64,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: Id,
    pub snippet: Option<Snippet>,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Id {
    pub kind: String,
    pub video_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snippet {
    pub title: String,
    pub description: String,
    pub channel_title: String,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub entries: Vec<PlaylistEntry>,
}

#[derive(Default, Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>
}

#[derive(Default, Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub struct SearchEntry {
    pub title: String,
    pub uploader: String,
    pub id: String
}

pub async fn search_song(input: &str) -> Result<Vec<SearchEntry>, String> {
    let key = get_api_key()?;
    let url = format!("https://youtube.googleapis.com/youtube/v3/search?part=snippet&order=relevance&q={}&type=video&key={}&maxResults=50", input, key);
    let response = reqwest::get(&url).await.map_err(stringify_error)?;
    if let Ok(result) = response.json::<YoutubeSearchResult>().await {
        let entries = result.items.into_iter().filter(|item| item.snippet.is_some()).map(|item| {
            let snippet = item.snippet.unwrap();
            SearchEntry {
                title: snippet.title.to_owned(),
                uploader: snippet.channel_title.to_owned(),
                id: item.id.video_id.to_owned()
            }
        }).collect();
        return Ok(entries);
    }
    Ok(vec![])
}

pub async fn similar_songs(id: &str) -> Result<Vec<SearchEntry>, String> {
    let key = get_api_key()?;
    let url = format!("https://youtube.googleapis.com/youtube/v3/search?part=snippet&order=relevance&type=video&key={}&maxResults=30&relatedToVideoId={}", key, id);
    let response = reqwest::get(&url).await.map_err(stringify_error)?;
    if let Ok(result) = response.json::<YoutubeSearchResult>().await {
        let entries = result.items.into_iter().filter(|item| item.snippet.is_some()).map(|item| {
            let snippet = item.snippet.unwrap();
            SearchEntry {
                title: snippet.title.to_owned(),
                uploader: snippet.channel_title.to_owned(),
                id: item.id.video_id.to_owned()
            }
        }).collect();
        return Ok(entries);
    }
    Ok(vec![])
}

pub async fn get_song_duration(id: &str) -> Result<Duration, String> {
    let key = get_api_key()?;
    let url = format!("https://youtube.googleapis.com/youtube/v3/videos?id={}&part=contentDetails&key={}&maxResults=30", id, key);
    let response = reqwest::get(&url).await.map_err(stringify_error)?;
    if let Ok(result) = response.json::<Value>().await {
        let duration = result["items"].as_array().unwrap()
            .get(0).unwrap().as_object().unwrap()
            .get("contentDetails").unwrap()
            .get("duration").unwrap()
            .as_str().unwrap();
        let re = Regex::new(r"^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+))S?$").unwrap();
        if let Some(captures) = re.captures(duration) {
            if captures.len() >= 3 {
                let hrs = captures.get(1).map_or(0, |m| m.as_str().parse::<u64>().unwrap());
                let min = captures.get(2).map_or(0, |m| m.as_str().parse::<u64>().unwrap());
                let sec = captures.get(3).map_or(0, |m| m.as_str().parse::<u64>().unwrap());
                return Ok(Duration::from_secs(sec + min * 60 + hrs * 60 * 60));
            }
        }
        return Ok(Duration::default());
    }
    Err("Cannot get song".to_owned())
}

