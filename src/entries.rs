//! What selecting a row does: builds the next list, starts playback, toggles settings.

use crate::app::*;
use crate::library::{self, Track};
use crate::screens::Slideshow;
use crate::settings::Kind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn dir_name(p: &Path) -> String {
    p.file_name().and_then(|n| n.to_str()).map(str::to_string).unwrap_or_else(|| p.display().to_string())
}

fn track_entries(tracks: &[Track]) -> Vec<Entry> {
    tracks.iter().enumerate().map(|(i, t)| Entry::new(t.title.clone(), false, Action::PlayAt(i))).collect()
}

fn group_entries(groups: Vec<(String, Vec<Track>)>, album_order: bool) -> Vec<Entry> {
    groups
        .into_iter()
        .map(|(name, mut tracks)| {
            if album_order {
                library::sort_album_order(&mut tracks);
            }
            let n = tracks.len();
            Entry::new(name.clone(), true, Action::OpenTracks(name, tracks)).with_trailing(format!("({n})"))
        })
        .collect()
}

fn empty(msg: &str) -> Vec<Entry> {
    vec![Entry::new(msg, false, Action::Noop)]
}

pub fn dir_entries(dir: &Path) -> Vec<Entry> {
    let list = library::list_videos_dir(dir);
    if list.is_empty() {
        return empty("No Movies Found");
    }
    list.into_iter()
        .map(|e| {
            if e.is_dir {
                Entry::new(e.name, true, Action::OpenDir(e.path))
            } else {
                let stem = e.path.file_stem().and_then(|s| s.to_str()).unwrap_or(&e.name).to_string();
                Entry::new(stem, false, Action::PlayVideo(e.path.clone())).with_thumb(format!("t:{}", e.path.display()), crate::texcache::Source::Thumb(e.path.display().to_string()))
            }
        })
        .collect()
}

fn chooser_entries(kind: Kind, dir: &Path) -> Vec<Entry> {
    let mut v = vec![];
    v.push(Entry::new("Use This Folder", false, Action::AddFolder(kind, dir.to_path_buf())));
    if let Some(parent) = dir.parent() {
        v.push(Entry::new("‹ Parent Folder", true, Action::Choose(kind, parent.to_path_buf())));
    }
    for e in library::list_subdirs(dir) {
        v.push(Entry::new(e.name, true, Action::Choose(kind, e.path)));
    }
    v
}

impl App {
    fn push_error(&mut self, now: f64, title: &str, message: &str) {
        self.sfx("Limit");
        self.set_screen(now, Screen::Error { title: title.into(), message: message.into() });
    }

    pub fn activate_sub(&mut self, now: f64) {
        let Some(item) = self.sub_items.get(self.sub_sel).cloned() else {
            self.sfx("Limit");
            return;
        };
        let root = self.roots[self.active_root].id;
        match item.id.as_str() {
            "music_now_playing" => {
                self.sfx("Selection");
                self.set_screen(now, Screen::NowPlaying);
            }
            "movies_folder" => {
                let folders = self.settings.folders(Kind::Movies);
                self.sfx("Selection");
                match folders.len() {
                    0 => self.push_error(now, "Movies Folder", "No movies folder was found.\nAdd one in Settings › Media Folders."),
                    1 => {
                        let entries = dir_entries(&folders[0]);
                        self.push_third(now, "Movies Folder".into(), entries);
                    }
                    _ => {
                        let entries = folders.into_iter().map(|f| Entry::new(dir_name(&f), true, Action::OpenDir(f))).collect();
                        self.push_third(now, "Movies Folder".into(), entries);
                    }
                }
            }
            "music_shuffle_songs" => {
                self.sfx("Selection");
                self.play_all(now, true);
            }
            "music_artists" | "music_albums" | "music_genres" | "music_composers" | "music_songs" | "music_audiobooks" | "music_playlists"
            | "music_music_videos" => self.open_music_category(now, &item.id, &item.title),
            "photos_library" => {
                self.sfx("Selection");
                if self.lib.albums_rx.is_some() {
                    self.toast(now, "Still scanning photos…");
                    return;
                }
                if self.lib.albums.is_empty() {
                    self.push_error(now, "Photos", "No photos were found.\nAdd a folder in Settings › Media Folders.");
                    return;
                }
                let mut entries = vec![];
                let all: Vec<PathBuf> = self.lib.albums.iter().flat_map(|a| a.photos.iter().map(|p| p.path.clone())).collect();
                entries.push(Entry::new("All Photos", false, Action::Slideshow(all.clone())).with_trailing(format!("({})", all.len())));
                for a in &self.lib.albums {
                    let paths: Vec<PathBuf> = a.photos.iter().map(|p| p.path.clone()).collect();
                    entries.push(Entry::new(a.name.clone(), false, Action::Slideshow(paths)).with_trailing(format!("({})", a.photos.len())));
                }
                self.push_third(now, "Photos".into(), entries);
            }
            "photos_last_12_months" => {
                let cutoff = SystemTime::now() - Duration::from_secs(365 * 24 * 3600);
                let paths: Vec<PathBuf> = self
                    .lib
                    .albums
                    .iter()
                    .flat_map(|a| a.photos.iter().filter(|p| p.modified >= cutoff).map(|p| p.path.clone()))
                    .collect();
                self.start_slideshow(now, paths, "No photos from the last 12 months.");
            }
            "photos_last_import" => {
                let mut all: Vec<&library::Photo> = self.lib.albums.iter().flat_map(|a| a.photos.iter()).collect();
                all.sort_by(|a, b| b.modified.cmp(&a.modified));
                let paths: Vec<PathBuf> = all.into_iter().take(24).map(|p| p.path.clone()).collect();
                self.start_slideshow(now, paths, "No photos were found.");
            }
            "settings_screensaver" => {
                self.settings.screen_saver = !self.settings.screen_saver;
                self.sfx("Selection");
                self.save_settings();
                self.refresh_sub_items();
            }
            "settings_quality" => {
                self.settings.video_quality = self.settings.video_quality.next();
                self.sfx("Selection");
                self.save_settings();
                self.refresh_sub_items();
            }
            "settings_soundeffects" => {
                self.settings.sound_effects = !self.settings.sound_effects;
                self.audio.sfx_enabled = self.settings.sound_effects;
                self.sfx("Selection");
                self.save_settings();
                self.refresh_sub_items();
            }
            "settings_media_folders" => {
                self.sfx("Selection");
                let entries = [Kind::Movies, Kind::Music, Kind::Photos]
                    .into_iter()
                    .map(|k| Entry::new(format!("{} Folders", k.label()), true, Action::Folders(k)))
                    .collect();
                self.push_third(now, "Media Folders".into(), entries);
            }
            "network_apps" => self.apps_menu(now),
            "network_add" => self.open_text_input(
                now,
                "Add Computer",
                "Type its IP address (for example 192.168.1.20) if it does not show up by itself",
                TextKind::AddComputer,
            ),
            id if id.starts_with("net:") => {
                let addr = id["net:".len()..].to_string();
                self.net_host(now, addr, item.title.clone());
            }
            _ => {
                if root == "podcasts" {
                    self.sfx("Limit");
                } else {
                    self.sfx("Limit");
                }
            }
        }
    }

    fn start_slideshow(&mut self, now: f64, paths: Vec<PathBuf>, empty_msg: &str) {
        if paths.is_empty() {
            self.push_error(now, "Photos", empty_msg);
            return;
        }
        self.sfx("Selection");
        let show = Slideshow::new(paths, now);
        self.ambient.start_default(&self.audio);
        self.set_screen(now, Screen::Slideshow(show));
    }

    fn open_music_category(&mut self, now: f64, id: &str, title: &str) {
        self.sfx("Selection");
        if self.lib.tracks_rx.is_some() {
            self.toast(now, "Still scanning your music…");
            return;
        }
        let tracks = &self.lib.tracks;
        let no_music = "No music was found.\nAdd a folder in Settings › Media Folders.";
        match id {
            "music_songs" => {
                if tracks.is_empty() {
                    return self.push_error(now, title, no_music);
                }
                let t = tracks.clone();
                self.push_third_with(now, title.into(), track_entries(&t), t);
            }
            "music_audiobooks" => {
                let t: Vec<Track> = tracks.iter().filter(|t| t.audiobook).cloned().collect();
                if t.is_empty() {
                    return self.push_error(now, title, "No audiobooks were found.\n(.m4b files, or tracks tagged Audiobook.)");
                }
                self.push_third_with(now, title.into(), track_entries(&t), t);
            }
            "music_artists" | "music_albums" | "music_genres" | "music_composers" => {
                if tracks.is_empty() {
                    return self.push_error(now, title, no_music);
                }
                let groups = match id {
                    "music_artists" => library::group_by(tracks, |t| t.artist.clone()),
                    "music_albums" => library::group_by(tracks, |t| t.album.clone()),
                    "music_genres" => library::group_by(tracks, |t| t.genre.clone()),
                    _ => library::group_by(tracks, |t| t.composer.clone()),
                };
                if groups.is_empty() {
                    return self.push_error(now, title, "Nothing found in your library for this category.");
                }
                let entries = group_entries(groups, id == "music_albums");
                self.push_third(now, title.into(), entries);
            }
            "music_playlists" => {
                let lists = library::scan_playlists(&self.settings.folders(Kind::Music));
                if lists.is_empty() {
                    return self.push_error(now, title, "No playlists were found.\n(.m3u / .m3u8 files in your music folders.)");
                }
                let entries = lists
                    .into_iter()
                    .map(|(name, files)| {
                        let tracks: Vec<Track> = files.iter().map(|f| library::read_track(f)).collect();
                        let n = tracks.len();
                        Entry::new(name.clone(), true, Action::OpenTracks(name, tracks)).with_trailing(format!("({n})"))
                    })
                    .collect();
                self.push_third(now, title.into(), entries);
            }
            _ => {
                // Music videos: videos inside the music folders.
                let folders = self.settings.folders(Kind::Music);
                let mut entries: Vec<Entry> = vec![];
                for f in &folders {
                    entries.extend(dir_entries(f).into_iter().filter(|e| !matches!(e.action, Action::Noop)));
                }
                if entries.is_empty() {
                    return self.push_error(now, title, "No music videos were found in your music folders.");
                }
                self.push_third(now, title.into(), entries);
            }
        }
    }

    pub fn play_all(&mut self, now: f64, shuffle: bool) {
        if self.lib.tracks_rx.is_some() {
            self.toast(now, "Still scanning your music…");
            return;
        }
        let mut tracks: Vec<Track> = self.lib.tracks.iter().filter(|t| !t.audiobook).cloned().collect();
        if tracks.is_empty() {
            return self.push_error(now, "Music", "No music was found.\nAdd a folder in Settings › Media Folders.");
        }
        if shuffle {
            use ::rand::seq::SliceRandom;
            tracks.shuffle(&mut ::rand::thread_rng());
        }
        self.start_playback(now, tracks, 0, shuffle);
    }

    pub fn start_playback(&mut self, now: f64, tracks: Vec<Track>, start: usize, shuffled: bool) {
        self.ambient.stop();
        self.player.play_queue(&self.audio, tracks, start, shuffled);
        if let Some(e) = self.player.error.clone() {
            return self.push_error(now, "Music", &e);
        }
        self.set_screen(now, Screen::NowPlaying);
    }

    pub fn activate_third(&mut self, now: f64) {
        let Some(third) = self.thirds.last() else { return };
        if third.loading.is_some() {
            return;
        }
        let Some(entry) = third.entries.get(third.sel).cloned() else { return };
        match entry.action {
            Action::Noop => self.sfx("Limit"),
            Action::OpenDir(dir) => {
                self.sfx("Selection");
                let entries = dir_entries(&dir);
                self.push_third(now, dir_name(&dir), entries);
            }
            Action::PlayVideo(path) => {
                self.sfx("Selection");
                self.play_video(now, path.display().to_string(), entry.label.clone());
            }
            Action::PlayPage(item) => self.play_page(now, item),
            Action::YtCategory(c) => self.yt_category(now, c),
            Action::YtHistory => self.yt_history(now),
            Action::YtSubs => self.yt_subs(now),
            Action::YtChannel(c) => self.yt_channel(now, c),
            Action::YtToggleSub(c) => self.yt_toggle_sub(c),
            Action::NetHost { addr, name } => self.net_host(now, addr, name),
            Action::NetBrowse { addr, lib, kind, p, title } => self.net_browse(now, addr, lib, kind, p, title),
            Action::AppsMenu => self.apps_menu(now),
            Action::AppOpen(id) => self.app_open(now, &id),
            Action::AppInstall(id) => self.app_install(&id),
            Action::AppRemove(id) => self.app_remove(now, &id),
            Action::AppPrompt(id) => self.app_prompt(now, &id),
            Action::Open(title, entries) => {
                self.sfx("Selection");
                self.push_third(now, title, entries);
            }
            Action::OpenTracks(title, tracks) => {
                self.sfx("Selection");
                let entries = track_entries(&tracks);
                self.push_third_with(now, title, entries, tracks);
            }
            Action::PlayAt(i) => {
                self.sfx("Selection");
                let tracks = self.thirds.last().map(|t| t.tracks.clone()).unwrap_or_default();
                self.start_playback(now, tracks, i, false);
            }
            Action::PlayAll { shuffle } => self.play_all(now, shuffle),
            Action::Slideshow(paths) => self.start_slideshow(now, paths, "This album has no photos."),
            Action::Folders(kind) => {
                self.sfx("Selection");
                let entries = self.folder_entries(kind);
                self.push_third(now, format!("{} Folders", kind.label()), entries);
            }
            Action::Choose(kind, dir) => {
                self.sfx("SelectionChange");
                let is_first = self.thirds.last().map(|t| t.title != dir_name(&dir) || true).unwrap_or(true);
                let _ = is_first;
                let entries = chooser_entries(kind, &dir);
                // Browsing deeper replaces the list so one Back leaves the chooser.
                if self.thirds.last().map(|t| t.title.starts_with("Choose:")).unwrap_or(false) {
                    self.replace_third(now, format!("Choose: {}", dir_name(&dir)), entries);
                } else {
                    self.push_third(now, format!("Choose: {}", dir_name(&dir)), entries);
                }
            }
            Action::AddFolder(kind, dir) => {
                self.sfx("Selection");
                if !self.settings.extra(kind).contains(&dir) {
                    self.settings.extra_mut(kind).push(dir);
                    self.save_settings();
                    self.reload_media();
                }
                // Back out of the chooser to the (refreshed) folder list.
                let entries = self.folder_entries(kind);
                self.swap(now, move |app, _| {
                    app.thirds.pop();
                    if let Some(t) = app.thirds.last_mut() {
                        t.entries = entries;
                        t.sel = 0;
                        app.header = t.title.clone();
                    }
                    app.views[2].reset();
                });
            }
            Action::RemoveFolder(kind, dir) => {
                self.sfx("Exit");
                self.settings.extra_mut(kind).retain(|d| *d != dir);
                self.save_settings();
                self.reload_media();
                let entries = self.folder_entries(kind);
                if let Some(t) = self.thirds.last_mut() {
                    t.entries = entries;
                    t.sel = t.sel.min(t.entries.len().saturating_sub(1));
                }
            }
            Action::Error(title, msg) => self.push_error(now, &title, &msg),
        }
    }

    fn folder_entries(&self, kind: Kind) -> Vec<Entry> {
        let mut entries = vec![Entry::new("Add Folder…", true, Action::Choose(kind, dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))))];
        let extra = self.settings.extra(kind).clone();
        for f in self.settings.folders(kind) {
            if extra.contains(&f) {
                entries.push(Entry::new(f.display().to_string(), false, Action::RemoveFolder(kind, f)).with_trailing("Remove"));
            } else {
                entries.push(Entry::new(f.display().to_string(), false, Action::Noop).with_trailing("Default"));
            }
        }
        entries
    }

    pub fn play_video(&mut self, now: f64, target: String, title: String) {
        self.ambient.stop();
        if self.player.has_session() {
            self.player.stop();
        }
        if crate::ffmpeg::available() {
            let v = crate::videoplayer::VideoPlayback::open(target, title, self.settings.video_quality);
            self.set_screen(now, Screen::Video(Box::new(v)));
            return;
        }
        // Without FFmpeg, fall back to an external player.
        match crate::video::launch(Path::new(&target)) {
            Ok((child, player)) => {
                self.set_screen(now, Screen::External { child: Some(child), player, title, started: now, error: None });
            }
            Err(_) => self.push_error(now, "Movies", "Playing video needs FFmpeg.\nInstall it (brew install ffmpeg / apt install ffmpeg)."),
        }
    }
}

fn menu_device() -> String {
    crate::menu::device_name()
}
