//! Scanning the user's media folders: music (with tags), videos and photos.

use lofty::prelude::*;
use lofty::tag::ItemKey;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

pub const AUDIO_EXT: &[&str] = &["mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "wav", "aif", "aiff"];
pub const VIDEO_EXT: &[&str] = &["mp4", "m4v", "mov", "mkv", "avi", "webm", "mpg", "mpeg", "wmv", "ts", "flv", "ogv"];
pub const PHOTO_EXT: &[&str] = &["jpg", "jpeg", "png"];

pub fn ext_of(p: &Path) -> String {
    p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

pub fn is_audio(p: &Path) -> bool {
    AUDIO_EXT.contains(&ext_of(p).as_str())
}
pub fn is_video(p: &Path) -> bool {
    VIDEO_EXT.contains(&ext_of(p).as_str())
}
pub fn is_photo(p: &Path) -> bool {
    PHOTO_EXT.contains(&ext_of(p).as_str())
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub composer: String,
    pub duration: f32,
    pub track_no: u32,
    pub disc_no: u32,
    pub audiobook: bool,
}

fn file_stem(p: &Path) -> String {
    p.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown").to_string()
}

pub fn read_track(path: &Path) -> Track {
    let mut t = Track {
        path: path.to_path_buf(),
        title: file_stem(path),
        artist: "Unknown Artist".into(),
        album: "Unknown Album".into(),
        genre: "Unknown Genre".into(),
        composer: String::new(),
        audiobook: ext_of(path) == "m4b",
        ..Default::default()
    };
    if let Ok(tagged) = lofty::read_from_path(path) {
        t.duration = tagged.properties().duration().as_secs_f32();
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(v) = tag.title() {
                if !v.trim().is_empty() {
                    t.title = v.to_string();
                }
            }
            if let Some(v) = tag.artist() {
                if !v.trim().is_empty() {
                    t.artist = v.to_string();
                }
            }
            if let Some(v) = tag.album() {
                if !v.trim().is_empty() {
                    t.album = v.to_string();
                }
            }
            if let Some(v) = tag.genre() {
                if !v.trim().is_empty() {
                    t.genre = v.to_string();
                }
            }
            if let Some(v) = tag.get_string(&ItemKey::Composer) {
                t.composer = v.to_string();
            }
            t.track_no = tag.track().unwrap_or(0);
            t.disc_no = tag.disk().unwrap_or(0);
            if t.genre.eq_ignore_ascii_case("audiobook") || t.genre.eq_ignore_ascii_case("audiobooks") {
                t.audiobook = true;
            }
        }
    }
    t
}

/// Embedded cover art bytes, if the file has any.
pub fn cover_art(path: &Path) -> Option<Vec<u8>> {
    let tagged = lofty::read_from_path(path).ok()?;
    for tag in tagged.tags() {
        if let Some(p) = tag.pictures().first() {
            return Some(p.data().to_vec());
        }
    }
    // Folder art next to the file.
    let dir = path.parent()?;
    for name in ["cover.jpg", "cover.png", "folder.jpg", "folder.png", "Cover.jpg", "Folder.jpg", "front.jpg", "albumart.jpg"] {
        if let Ok(b) = std::fs::read(dir.join(name)) {
            return Some(b);
        }
    }
    None
}

pub fn scan_music(folders: &[PathBuf]) -> Vec<Track> {
    let mut out = vec![];
    for f in folders {
        for e in WalkDir::new(f).follow_links(true).into_iter().filter_entry(|e| !is_hidden(&e.file_name().to_string_lossy())).flatten() {
            if e.file_type().is_file() && is_audio(e.path()) {
                out.push(read_track(e.path()));
            }
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

pub fn sort_album_order(tracks: &mut [Track]) {
    tracks.sort_by(|a, b| (a.disc_no, a.track_no, a.title.to_lowercase()).cmp(&(b.disc_no, b.track_no, b.title.to_lowercase())));
}

/// Groups tracks by a key (artist, album, genre, composer), keys sorted case-insensitively.
pub fn group_by<F: Fn(&Track) -> String>(tracks: &[Track], key: F) -> Vec<(String, Vec<Track>)> {
    let mut map: BTreeMap<String, (String, Vec<Track>)> = BTreeMap::new();
    for t in tracks {
        let k = key(t);
        if k.is_empty() {
            continue;
        }
        map.entry(k.to_lowercase()).or_insert_with(|| (k.clone(), vec![])).1.push(t.clone());
    }
    map.into_values().collect()
}

#[derive(Clone, Debug)]
pub struct FsEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Folders and playable videos in `dir`, folders first.
pub fn list_videos_dir(dir: &Path) -> Vec<FsEntry> {
    let mut v: Vec<FsEntry> = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if is_hidden(&name) {
                continue;
            }
            let path = e.path();
            let is_dir = path.is_dir();
            if is_dir || is_video(&path) {
                v.push(FsEntry { name, path, is_dir });
            }
        }
    }
    v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    v
}

/// Sub-folders of `dir` (for the folder chooser).
pub fn list_subdirs(dir: &Path) -> Vec<FsEntry> {
    let mut v: Vec<FsEntry> = vec![];
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if is_hidden(&name) {
                continue;
            }
            let path = e.path();
            if path.is_dir() {
                v.push(FsEntry { name, path, is_dir: true });
            }
        }
    }
    v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    v
}

#[derive(Clone, Debug)]
pub struct Photo {
    pub path: PathBuf,
    pub modified: SystemTime,
}

#[derive(Clone, Debug)]
pub struct Album {
    pub name: String,
    pub photos: Vec<Photo>,
}

pub fn scan_photos(folders: &[PathBuf]) -> Vec<Album> {
    let mut map: BTreeMap<PathBuf, Vec<Photo>> = BTreeMap::new();
    for f in folders {
        for e in WalkDir::new(f).follow_links(true).into_iter().filter_entry(|e| !is_hidden(&e.file_name().to_string_lossy())).flatten() {
            if e.file_type().is_file() && is_photo(e.path()) {
                let modified = e.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH);
                let dir = e.path().parent().map(|p| p.to_path_buf()).unwrap_or_default();
                map.entry(dir).or_default().push(Photo { path: e.path().to_path_buf(), modified });
            }
        }
    }
    let mut albums: Vec<Album> = map
        .into_iter()
        .map(|(dir, mut photos)| {
            photos.sort_by(|a, b| a.path.cmp(&b.path));
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("Photos").to_string();
            Album { name, photos }
        })
        .collect();
    albums.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    albums
}

pub fn scan_playlists(folders: &[PathBuf]) -> Vec<(String, Vec<PathBuf>)> {
    let mut out = vec![];
    for f in folders {
        for e in WalkDir::new(f).max_depth(3).into_iter().flatten() {
            let ext = ext_of(e.path());
            if e.file_type().is_file() && (ext == "m3u" || ext == "m3u8") {
                if let Ok(text) = std::fs::read_to_string(e.path()) {
                    let base = e.path().parent().unwrap_or(Path::new("."));
                    let files: Vec<PathBuf> = text
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty() && !l.starts_with('#'))
                        .map(|l| {
                            let p = PathBuf::from(l);
                            if p.is_absolute() { p } else { base.join(p) }
                        })
                        .filter(|p| p.is_file())
                        .collect();
                    out.push((file_stem(e.path()), files));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    out
}
