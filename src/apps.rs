//! "Apps": small installable services shown under Network > Apps. They use `yt-dlp` to search
//! and resolve streams, which the in-app player then plays through FFmpeg.

use crate::ffmpeg;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppKind {
    /// Search a site through yt-dlp's `<prefix>search<N>:` extractors.
    Search(&'static str),
    /// Paste any link yt-dlp understands.
    OpenLink,
}

pub struct AppDef {
    pub id: &'static str,
    pub name: &'static str,
    pub blurb: &'static str,
    pub kind: AppKind,
}

pub fn catalog() -> Vec<AppDef> {
    vec![
        AppDef { id: "youtube", name: "YouTube", blurb: "Search and watch videos", kind: AppKind::Search("ytsearch") },
        AppDef { id: "soundcloud", name: "SoundCloud", blurb: "Search and play music", kind: AppKind::Search("scsearch") },
        AppDef { id: "link", name: "Open Link", blurb: "Play a link from almost any video site", kind: AppKind::OpenLink },
    ]
}

pub fn def(id: &str) -> Option<AppDef> {
    catalog().into_iter().find(|a| a.id == id)
}

pub fn ytdlp() -> Option<std::path::PathBuf> {
    ffmpeg::tool("yt-dlp").or_else(|| ffmpeg::tool("youtube-dl"))
}

const MISSING: &str = "This app needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp) and try again.";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultItem {
    pub title: String,
    pub url: String,
    pub by: String,
    pub duration: f32,
    pub thumb: Option<String>,
    #[serde(default)]
    pub channel_url: Option<String>,
}

/// A YouTube channel the user searched for or subscribed to.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelRef {
    pub name: String,
    pub url: String,
    pub thumb: Option<String>,
}

/// The YouTube app's browse lists, built from YouTube's own sorted search results.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Category {
    Featured,
    MostViewed,
    MostRecent,
    TopRated,
}

impl Category {
    pub const ALL: [Category; 4] = [Category::Featured, Category::MostViewed, Category::MostRecent, Category::TopRated];

    pub fn label(self) -> &'static str {
        match self {
            Category::Featured => "Featured",
            Category::MostViewed => "Most Viewed",
            Category::MostRecent => "Most Recent",
            Category::TopRated => "Top Rated",
        }
    }

    pub fn url(self) -> String {
        // sp= is YouTube's sort/filter code: views this week / views / upload date / rating (videos only).
        let sp = match self {
            Category::Featured => "CAMSBAgDEAE%253D",
            Category::MostViewed => "CAMSAhAB",
            Category::MostRecent => "CAISAhAB",
            Category::TopRated => "CAESAhAB",
        };
        format!("https://www.youtube.com/results?search_query=official&sp={sp}")
    }
}

pub fn search(prefix: &str, query: &str) -> Result<Vec<ResultItem>, String> {
    list(&format!("{prefix}12:{query}"), 12)
}

/// Lists videos from a search URL / playlist without resolving each one.
pub fn list(source: &str, n: usize) -> Result<Vec<ResultItem>, String> {
    let bin = ytdlp().ok_or(MISSING)?;
    let out = Command::new(bin)
        .args(["--flat-playlist", "--playlist-end", &n.to_string(), "-j", "--no-warnings", "--ignore-errors", source])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let prefix = if source.contains("youtube.com") || source.starts_with("ytsearch") { "ytsearch" } else { "" };
    let mut items = vec![];
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
        let id = v["id"].as_str().unwrap_or("");
        let url = v["webpage_url"].as_str().or_else(|| v["url"].as_str()).unwrap_or("").to_string();
        if url.is_empty() || url.contains("/channel/") || url.contains("/@") || (url.contains("list=") && !url.contains("watch?v=")) {
            continue;
        }
        // YouTube thumbnails by id are plain JPEGs; other sites' thumbnail lists may be WebP, which we skip.
        let thumb = if prefix == "ytsearch" && !id.is_empty() {
            Some(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"))
        } else {
            v["thumbnails"]
                .as_array()
                .and_then(|t| t.iter().rev().filter_map(|x| x["url"].as_str()).find(|u| u.contains(".jpg") || u.contains(".png")))
                .map(str::to_string)
        };
        items.push(ResultItem {
            title: v["title"].as_str().unwrap_or("Untitled").to_string(),
            url,
            by: v["uploader"].as_str().or_else(|| v["channel"].as_str()).unwrap_or("").to_string(),
            duration: v["duration"].as_f64().unwrap_or(0.0) as f32,
            thumb,
            channel_url: v["channel_url"].as_str().or_else(|| v["uploader_url"].as_str()).map(str::to_string),
        });
    }
    if items.is_empty() {
        return Err("No results. Check your connection or try another search.".into());
    }
    Ok(items)
}

/// Channels matching a search (YouTube's "channel" result filter).
pub fn list_channels(query: &str) -> Result<Vec<ChannelRef>, String> {
    let bin = ytdlp().ok_or(MISSING)?;
    let q: String = query.split_whitespace().collect::<Vec<_>>().join("+");
    let url = format!("https://www.youtube.com/results?search_query={q}&sp=EgIQAg%253D%253D");
    let out = Command::new(bin)
        .args(["--flat-playlist", "--playlist-end", "12", "-j", "--no-warnings", "--ignore-errors", &url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let mut v = vec![];
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Ok(j) = serde_json::from_str::<Value>(line) else { continue };
        let u = j["url"].as_str().or_else(|| j["webpage_url"].as_str()).unwrap_or("");
        if !(u.contains("/channel/") || u.contains("/@") || u.contains("/user/") || u.contains("/c/")) {
            continue;
        }
        let thumb = j["thumbnails"].as_array().and_then(|t| t.iter().rev().find_map(|x| x["url"].as_str())).map(|t| if let Some(r) = t.strip_prefix("//") { format!("https://{r}") } else { t.to_string() });
        v.push(ChannelRef { name: j["title"].as_str().or_else(|| j["channel"].as_str()).unwrap_or("Channel").to_string(), url: u.to_string(), thumb });
    }
    if v.is_empty() {
        return Err("No channels found.".into());
    }
    Ok(v)
}

pub fn channel_videos(channel_url: &str) -> Result<Vec<ResultItem>, String> {
    list(&format!("{}/videos", channel_url.trim_end_matches('/')), 24)
}

/// One media input FFmpeg can read directly (with the HTTP headers the site wants).
#[derive(Clone, Debug)]
pub struct Input {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

/// Everything needed to stream a page's video: separate video and audio inputs, plus its properties.
#[derive(Clone, Debug)]
pub struct Stream {
    pub video: Option<Input>,
    pub audio: Option<Input>,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub duration: f32,
}

fn fmt_input(f: &Value) -> Option<Input> {
    let url = f["url"].as_str()?.to_string();
    let headers = f["http_headers"].as_object().map(|h| h.iter().filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string()))).collect()).unwrap_or_default();
    Some(Input { url, headers })
}

fn range_ok(i: &Input, range: &str) -> bool {
    let mut req = ureq::get(&i.url).timeout(std::time::Duration::from_secs(10)).set("Range", range);
    for (k, v) in &i.headers {
        req = req.set(k, v);
    }
    matches!(req.call(), Ok(r) if r.status() == 200 || r.status() == 206)
}

/// Checks the way FFmpeg will read it: an open-ended range from the start and from the middle of the file
/// (some YouTube clients only accept small chunked ranges, which FFmpeg cannot use).
fn reachable(i: &Input) -> bool {
    if i.url.contains(".m3u8") || i.url.contains("/manifest/") {
        return true;
    }
    range_ok(i, "bytes=0-") && range_ok(i, "bytes=3000000-")
}

/// Finds live stream URLs for a page (no downloading). YouTube refuses some clients per video, so several
/// are tried and each result is checked with a tiny range request before it is used.
pub fn resolve_stream(url: &str, max_height: u32, status: &std::sync::Arc<std::sync::Mutex<String>>) -> Result<Stream, String> {
    let bin = ytdlp().ok_or(MISSING)?;
    let is_sc = url.contains("soundcloud.com");
    let is_yt = url.contains("youtube.com") || url.contains("youtu.be");
    let fmt = if is_sc {
        "bestaudio/b".to_string()
    } else {
        format!("bv*[height<={max_height}]+ba/b[height<={max_height}]/b/bestaudio")
    };
    let clients: Vec<Option<&str>> = if is_yt { vec![Some("web_embedded"), Some("web_embedded"), None, Some("tv"), Some("web_safari"), Some("mweb"), Some("android_vr")] } else { vec![None] };
    let mut last = String::from("Couldn't get a stream for that link.");
    let mut fallback: Option<Stream> = None;
    for (n, client) in clients.iter().enumerate() {
        if let Ok(mut s) = status.lock() {
            *s = if n == 0 { "Contacting the site…".into() } else { format!("Trying another route… ({})", n + 1) };
        }
        let mut cmd = Command::new(&bin);
        cmd.args(["-j", "--no-playlist", "--no-warnings", "-f", &fmt]);
        if let Some(c) = client {
            cmd.args(["--extractor-args", &format!("youtube:player_client={c}")]);
        }
        let out = cmd.arg(url).stdin(Stdio::null()).stderr(Stdio::piped()).output().map_err(|e| e.to_string())?;
        let Ok(j) = serde_json::from_slice::<Value>(&out.stdout) else {
            if let Some(l) = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| l.contains("ERROR")) {
                last = l.trim().chars().take(110).collect();
            }
            continue;
        };
        let formats: Vec<Value> = j["requested_formats"].as_array().cloned().unwrap_or_else(|| vec![j.clone()]);
        let mut st = Stream { video: None, audio: None, width: 0, height: 0, fps: 0.0, duration: j["duration"].as_f64().unwrap_or(0.0) as f32 };
        for f in &formats {
            let has_v = f["vcodec"].as_str().map(|v| v != "none").unwrap_or(false) || (f["vcodec"].is_null() && f["width"].is_number());
            let has_a = f["acodec"].as_str().map(|a| a != "none").unwrap_or(false);
            if has_v && st.video.is_none() {
                st.video = fmt_input(f);
                st.width = f["width"].as_u64().unwrap_or(0) as u32;
                st.height = f["height"].as_u64().unwrap_or(0) as u32;
                st.fps = f["fps"].as_f64().unwrap_or(30.0) as f32;
                if has_a && st.audio.is_none() {
                    st.audio = st.video.clone();
                }
            } else if has_a && st.audio.is_none() {
                st.audio = fmt_input(f);
            }
        }
        if st.video.is_none() && st.audio.is_none() {
            continue;
        }
        let ok = st.video.as_ref().map(reachable).unwrap_or(true) && st.audio.as_ref().map(reachable).unwrap_or(true);
        if ok {
            return Ok(st);
        }
        // Not fully verified, but a small range worked: keep it as a last resort.
        let lenient = st.video.as_ref().map(|i| range_ok(i, "bytes=0-1")).unwrap_or(true) && st.audio.as_ref().map(|i| range_ok(i, "bytes=0-1")).unwrap_or(true);
        if lenient && fallback.is_none() {
            fallback = Some(st);
        }
        last = "The site refused the stream (HTTP 403).".into();
    }
    fallback.ok_or(last)
}
