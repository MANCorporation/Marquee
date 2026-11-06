//! Thin wrappers around the `ffmpeg` / `ffprobe` command-line tools (video decoding, probing, thumbnails).

use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Finds a tool on PATH or in the usual package-manager locations (a Finder-launched app has a short PATH).
pub fn tool(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    for extra in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/snap/bin", "/home/linuxbrew/.linuxbrew/bin"] {
        dirs.push(PathBuf::from(extra));
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

pub fn available() -> bool {
    tool("ffmpeg").is_some() && tool("ffprobe").is_some()
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub duration: f32,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub has_audio: bool,
    pub has_video: bool,
}

fn parse_rate(s: &str) -> f32 {
    match s.split_once('/') {
        Some((a, b)) => {
            let (a, b): (f32, f32) = (a.parse().unwrap_or(0.0), b.parse().unwrap_or(1.0));
            if b > 0.0 { a / b } else { 0.0 }
        }
        None => s.parse().unwrap_or(0.0),
    }
}

pub fn probe(src: &str) -> Result<Probe, String> {
    let ffprobe = tool("ffprobe").ok_or("ffprobe was not found. Install FFmpeg to play videos in the app.")?;
    let out = Command::new(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", src])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|_| "This file could not be read.".to_string())?;
    let streams = v.get("streams").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let mut p = Probe { duration: 0.0, width: 0, height: 0, fps: 0.0, has_audio: false, has_video: false };
    p.duration = v.pointer("/format/duration").and_then(|d| d.as_str()).and_then(|d| d.parse().ok()).unwrap_or(0.0);
    for s in &streams {
        match s.get("codec_type").and_then(|c| c.as_str()) {
            Some("video") if !p.has_video && s.pointer("/disposition/attached_pic").and_then(|a| a.as_i64()) != Some(1) => {
                p.has_video = true;
                p.width = s.get("width").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
                p.height = s.get("height").and_then(|w| w.as_u64()).unwrap_or(0) as u32;
                p.fps = s.get("avg_frame_rate").and_then(|r| r.as_str()).map(parse_rate).filter(|f| *f > 1.0).unwrap_or_else(|| {
                    s.get("r_frame_rate").and_then(|r| r.as_str()).map(parse_rate).unwrap_or(24.0)
                });
            }
            Some("audio") => p.has_audio = true,
            _ => {}
        }
    }
    if !p.has_video && !p.has_audio {
        return Err("No playable audio or video was found in this file.".into());
    }
    Ok(p)
}

static ACTIVE: AtomicUsize = AtomicUsize::new(0);

struct Slot;
impl Slot {
    fn acquire() -> Slot {
        loop {
            let n = ACTIVE.load(Ordering::SeqCst);
            if n < 3 && ACTIVE.compare_exchange(n, n + 1, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                return Slot;
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

fn cache_path(src: &str) -> PathBuf {
    let mut h = DefaultHasher::new();
    src.hash(&mut h);
    if let Ok(m) = std::fs::metadata(src).and_then(|m| m.modified()) {
        m.hash(&mut h);
    }
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("marquee").join("thumbs").join(format!("{:016x}.jpg", h.finish()))
}

/// A JPEG frame from near the start of the video (cached on disk). Works for local files and URLs.
pub fn thumbnail(src: &str) -> Option<Vec<u8>> {
    let cache = cache_path(src);
    if let Ok(b) = std::fs::read(&cache) {
        if !b.is_empty() {
            return Some(b);
        }
    }
    let ffmpeg = tool("ffmpeg")?;
    let _slot = Slot::acquire();
    for ss in ["8", "1", "0"] {
        let out = Command::new(&ffmpeg)
            .args(["-v", "error", "-ss", ss, "-i", src, "-frames:v", "1", "-an", "-vf", "scale=640:-2", "-q:v", "4", "-f", "image2pipe", "-vcodec", "mjpeg", "-"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        if !out.stdout.is_empty() {
            if let Some(dir) = cache.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&cache, &out.stdout);
            return Some(out.stdout);
        }
    }
    None
}
