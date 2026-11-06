//! Persistent settings and the folders media is read from.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Movies,
    Music,
    Photos,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Movies => "Movies",
            Kind::Music => "Music",
            Kind::Photos => "Photos",
        }
    }
}

/// How much video quality to stream and decode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum Quality {
    /// Best stream available (up to 4K), shown at up to 1080p.
    #[default]
    Auto,
    /// Best stream, shown at the window's native size (up to 4K): needs a fast computer.
    Ultra,
    High,
    Low,
}

impl Quality {
    pub fn label(self) -> &'static str {
        match self {
            Quality::Auto => "Auto",
            Quality::Ultra => "Ultra (4K)",
            Quality::High => "1080p",
            Quality::Low => "720p",
        }
    }
    pub fn next(self) -> Quality {
        match self {
            Quality::Auto => Quality::Ultra,
            Quality::Ultra => Quality::High,
            Quality::High => Quality::Low,
            Quality::Low => Quality::Auto,
        }
    }
    /// Tallest stream to request.
    pub fn stream_height(self) -> u32 {
        match self {
            Quality::Auto | Quality::Ultra => 2160,
            Quality::High => 1080,
            Quality::Low => 720,
        }
    }
    /// Tallest picture to decode into (the frames travel as raw RGBA, so this bounds the work).
    pub fn output_height(self) -> u32 {
        match self {
            Quality::Ultra => 2160,
            Quality::Auto | Quality::High => 1080,
            Quality::Low => 720,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Settings {
    pub video_quality: Quality,
    pub sound_effects: bool,
    pub screen_saver: bool,
    pub extra_movie_folders: Vec<PathBuf>,
    pub extra_music_folders: Vec<PathBuf>,
    pub extra_photo_folders: Vec<PathBuf>,
    pub installed_apps: Vec<String>,
    pub history: Vec<crate::apps::ResultItem>,
    pub subs: Vec<crate::apps::ChannelRef>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            video_quality: Quality::Auto,
            sound_effects: true,
            screen_saver: true,
            extra_movie_folders: vec![],
            extra_music_folders: vec![],
            extra_photo_folders: vec![],
            installed_apps: vec!["youtube".into()],
            history: vec![],
            subs: vec![],
        }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("marquee").join("settings.json"))
}

impl Settings {
    pub fn load() -> Settings {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(s) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }

    pub fn extra(&self, kind: Kind) -> &Vec<PathBuf> {
        match kind {
            Kind::Movies => &self.extra_movie_folders,
            Kind::Music => &self.extra_music_folders,
            Kind::Photos => &self.extra_photo_folders,
        }
    }

    pub fn extra_mut(&mut self, kind: Kind) -> &mut Vec<PathBuf> {
        match kind {
            Kind::Movies => &mut self.extra_movie_folders,
            Kind::Music => &mut self.extra_music_folders,
            Kind::Photos => &mut self.extra_photo_folders,
        }
    }

    /// Default locations (the user's folders and `Movies`/`Music`/`Pictures` on mounted volumes),
    /// then anything added in Settings > Media Folders, then `MARQUEE_*` environment overrides.
    pub fn folders(&self, kind: Kind) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = vec![];
        // MARQUEE_NO_DEFAULTS=1 skips the user's own folders (handy for demos and tests).
        let defaults = std::env::var_os("MARQUEE_NO_DEFAULTS").is_none();
        let home = if !defaults { None } else { match kind {
            Kind::Movies => dirs::video_dir().or_else(|| dirs::home_dir().map(|h| h.join("Movies"))),
            Kind::Music => dirs::audio_dir().or_else(|| dirs::home_dir().map(|h| h.join("Music"))),
            Kind::Photos => dirs::picture_dir().or_else(|| dirs::home_dir().map(|h| h.join("Pictures"))),
        } };
        out.extend(home);
        let sub = match kind {
            Kind::Movies => "Movies",
            Kind::Music => "Music",
            Kind::Photos => "Pictures",
        };
        if defaults {
            for root in volume_roots() {
                out.push(root.join(sub));
            }
            out.extend(self.extra(kind).iter().cloned());
        }
        let var = match kind {
            Kind::Movies => "MARQUEE_MOVIES",
            Kind::Music => "MARQUEE_MUSIC",
            Kind::Photos => "MARQUEE_PHOTOS",
        };
        if let Some(v) = std::env::var_os(var) {
            out.extend(std::env::split_paths(&v));
        }
        let mut seen = std::collections::HashSet::new();
        out.into_iter().filter(|p| p.is_dir() && seen.insert(p.canonicalize().unwrap_or(p.clone()))).collect()
    }
}

/// Mounted volumes whose top level may hold `Movies`/`Music` folders.
fn volume_roots() -> Vec<PathBuf> {
    let mut roots = vec![];
    let mut scan = |dir: PathBuf| {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                roots.push(e.path());
            }
        }
    };
    #[cfg(target_os = "macos")]
    scan(PathBuf::from("/Volumes"));
    #[cfg(not(target_os = "macos"))]
    {
        scan(PathBuf::from("/media"));
        scan(PathBuf::from("/mnt"));
        if let Some(user) = std::env::var_os("USER") {
            scan(PathBuf::from("/media").join(&user));
            scan(PathBuf::from("/run/media").join(user));
        }
    }
    roots
}
