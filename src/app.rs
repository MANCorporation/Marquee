//! Application state, input and navigation (the Swift `MenuView` and its `+InputHandling`/`+MenuNavigation`).

use crate::anim::Tween;
use crate::audio::{Ambient, Audio, Player};
use crate::gfx::Gfx;
use crate::library::{self, Album, Track};
use crate::menu::{self, RootItem, SubItem};
use crate::settings::{Kind, Settings};
use crate::texcache::{Source, TexCache};
use macroquad::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

// Layout constants, as in MenuView.swift.
pub const ICON_SIZE: f32 = 400.0;
pub const MENU_WIDTH: f32 = 900.0;
pub const SELECTION_BOX_HEIGHT: f32 = 75.0;
pub const ROW_SPACING: f32 = 5.0;
pub const DEFAULT_VISIBLE_ROWS: usize = 9;
pub const THIRD_VISIBLE_ROWS: usize = 10;
pub const STICKY_ROW: usize = 3;
pub const DIVIDER_GAP: f32 = 14.0;
pub const LIST_BOTTOM_INSET: f32 = 200.0;
pub const ARC_RADIUS: f32 = 550.0;
pub const SCALE_REDUCTION: f32 = 0.3;
pub const SELECTED_MULT: f32 = 1.65;
pub const LANDED_SCALE: f32 = 0.2;
pub const ENTRY_OFFSET: f32 = 200.0 * 1.5;
pub const FLIGHT: f32 = 0.5;
pub const SELECTION_ANIM: f32 = 0.30;
pub const SAVER_IDLE_SECS: f64 = 60.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Back,
    Space,
    Backspace,
    Channel,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Root,
    Sub,
    Third,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub title: String,
    pub leads: bool,
    pub trailing: Option<String>,
    pub divider: bool,
    pub glyph: bool,
    pub thumb: Option<(String, Source)>,
    pub icon: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub enum Action {
    Noop,
    OpenDir(PathBuf),
    PlayVideo(PathBuf),
    PlayPage(crate::apps::ResultItem),
    YtCategory(crate::apps::Category),
    YtHistory,
    YtSubs,
    YtChannel(crate::apps::ChannelRef),
    YtToggleSub(crate::apps::ChannelRef),
    NetHost { addr: String, name: String },
    NetBrowse { addr: String, lib: String, kind: String, p: String, title: String },
    AppsMenu,
    AppOpen(String),
    AppInstall(String),
    AppRemove(String),
    AppPrompt(String),
    Open(String, Vec<Entry>),
    OpenTracks(String, Vec<Track>),
    PlayAt(usize),
    PlayAll { shuffle: bool },
    Slideshow(Vec<PathBuf>),
    Folders(Kind),
    Choose(Kind, PathBuf),
    AddFolder(Kind, PathBuf),
    RemoveFolder(Kind, PathBuf),
    Error(String, String),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub label: String,
    pub trailing: Option<String>,
    pub leads: bool,
    pub action: Action,
    pub thumb: Option<(String, Source)>,
}

impl Entry {
    pub fn new(label: impl Into<String>, leads: bool, action: Action) -> Entry {
        Entry { label: label.into(), trailing: None, leads, action, thumb: None }
    }
    pub fn with_thumb(mut self, key: String, src: Source) -> Entry {
        self.thumb = Some((key, src));
        self
    }
    pub fn with_trailing(mut self, t: impl Into<String>) -> Entry {
        self.trailing = Some(t.into());
        self
    }
}

pub struct ThirdLevel {
    pub title: String,
    pub entries: Vec<Entry>,
    pub sel: usize,
    /// Tracks behind a song list (entries use `Action::PlayAt`).
    pub tracks: Vec<Track>,
    /// Set while a network request fills the list in.
    pub loading: Option<Receiver<Result<Vec<Entry>, String>>>,
}

#[derive(Default)]
pub struct ListView {
    pub sel_y: Option<Tween>,
    pub scroll: Option<Tween>,
    pub last_target: Option<(f32, f32)>,
}

impl ListView {
    pub fn reset(&mut self) {
        *self = ListView::default();
    }
}

#[derive(Clone, Debug)]
pub enum TextKind {
    AddComputer,
    Search(String),
    ChannelSearch,
    OpenLink,
}

pub enum Screen {
    Menu,
    NowPlaying,
    Slideshow(crate::screens::Slideshow),
    Saver(crate::screens::Saver),
    Error { title: String, message: String },
    TextInput { prompt: String, hint: String, text: String, kind: TextKind },
    Video(Box<crate::videoplayer::VideoPlayback>),
    External { child: Option<std::process::Child>, player: &'static str, title: String, started: f64, error: Option<String> },
}

pub enum YtSlot {
    Loading(Receiver<Result<Vec<crate::apps::ResultItem>, String>>),
    Ready(Vec<crate::apps::ResultItem>),
    Failed,
}

pub struct Library {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub tracks_rx: Option<Receiver<Vec<Track>>>,
    pub albums_rx: Option<Receiver<Vec<Album>>>,
}

pub struct App {
    pub g: Gfx,
    pub audio: Audio,
    pub player: Player,
    pub ambient: Ambient,
    pub settings: Settings,
    pub lib: Library,
    pub tex: TexCache,
    pub roots: Vec<RootItem>,
    pub sel: usize,
    pub phase: Tween,
    pub enter: Tween,
    pub level: Level,
    pub active_root: usize,
    pub sub_items: Vec<SubItem>,
    pub sub_sel: usize,
    pub thirds: Vec<ThirdLevel>,
    pub root_alpha: Tween,
    pub sub_alpha: Tween,
    pub header: String,
    pub views: [ListView; 3],
    pub fade: Tween,
    pub pending: Option<Box<dyn FnOnce(&mut App, f64)>>,
    pub screen: Screen,
    pub last_input: f64,
    pub busy_until: f64,
    pub quit: bool,
    pub linear_sweep: bool,
    pub saver_idle: f64,
    pub toast: Option<(String, f64)>,
    pub pending_error: Option<(String, String)>,
    pub disc: crate::net::Discovery,
    pub last_net_refresh: f64,
    pub yt_cache: std::collections::HashMap<crate::apps::Category, YtSlot>,
    pub wall_id: String,
    pub wall_pos: Tween,
    pub wall_fade: Tween,
}

impl App {
    pub fn new() -> App {
        let settings = Settings::load();
        let mut audio = Audio::new();
        audio.sfx_enabled = settings.sound_effects;
        let (tx, rx) = std::sync::mpsc::channel();
        let music_folders = settings.folders(Kind::Music);
        std::thread::spawn(move || {
            let _ = tx.send(library::scan_music(&music_folders));
        });
        let (ptx, prx) = std::sync::mpsc::channel();
        let photo_folders = settings.folders(Kind::Photos);
        std::thread::spawn(move || {
            let _ = ptx.send(library::scan_photos(&photo_folders));
        });
        let mut app = App {
            g: Gfx::load(),
            audio,
            player: Player::new(),
            ambient: Ambient::new(),
            settings,
            lib: Library { tracks: vec![], albums: vec![], tracks_rx: Some(rx), albums_rx: Some(prx) },
            tex: TexCache::new(48),
            roots: menu::root_items(),
            sel: 0,
            phase: Tween::new(0.0),
            enter: Tween::new(0.0),
            level: Level::Root,
            active_root: 0,
            sub_items: vec![],
            sub_sel: 0,
            thirds: vec![],
            root_alpha: Tween::new(1.0),
            sub_alpha: Tween::new(0.0),
            header: "Marquee".into(),
            views: Default::default(),
            fade: Tween::new(1.0),
            pending: None,
            screen: Screen::Menu,
            last_input: 0.0,
            busy_until: 0.0,
            quit: false,
            linear_sweep: false,
            saver_idle: std::env::var("MARQUEE_SAVER_IDLE").ok().and_then(|v| v.parse().ok()).unwrap_or(SAVER_IDLE_SECS),
            toast: None,
            pending_error: None,
            disc: crate::net::Discovery::start(),
            last_net_refresh: 0.0,
            yt_cache: Default::default(),
            wall_id: String::new(),
            wall_pos: Tween::new(0.0),
            wall_fade: Tween::new(1.0),
        };
        app.fade.go(0.0, 0.0, 1.2);
        app
    }

    // ----- helpers -------------------------------------------------------------------

    pub fn sfx(&self, name: &str) {
        self.audio.play_sfx(name);
    }

    pub fn toast(&mut self, now: f64, msg: impl Into<String>) {
        self.toast = Some((msg.into(), now + 3.0));
    }

    /// Fades to black, runs `f`, then fades back in.
    pub fn swap(&mut self, now: f64, f: impl FnOnce(&mut App, f64) + 'static) {
        if self.pending.is_some() {
            return;
        }
        self.fade.go(now, 1.0, 0.22);
        self.pending = Some(Box::new(f));
    }

    pub fn set_screen(&mut self, now: f64, screen: Screen) {
        self.swap(now, move |app, _| app.screen = screen);
    }

    pub fn back_to_menu(&mut self, now: f64) {
        self.swap(now, |app, _| {
            app.screen = Screen::Menu;
        });
    }

    pub fn sub_rows(&self) -> Vec<Row> {
        self.sub_items
            .iter()
            .map(|s| Row {
                title: s.title.clone(),
                leads: s.leads_to_menu,
                trailing: s.trailing_text.clone(),
                divider: s.top_divider,
                glyph: false,
                thumb: None,
                icon: sub_icon(&s.id),
            })
            .collect()
    }

    pub fn third_rows(&self) -> Vec<Row> {
        self.thirds
            .last()
            .map(|t| {
                if t.loading.is_some() {
                    return vec![Row { title: "Loading…".into(), leads: false, trailing: None, divider: false, glyph: false, thumb: None, icon: None }];
                }
                t.entries
                    .iter()
                    .map(|e| Row { title: e.label.clone(), leads: e.leads, trailing: e.trailing.clone(), divider: false, glyph: false, thumb: e.thumb.clone(), icon: icon_for(e) })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn root_rows(&self) -> Vec<Row> {
        self.roots
            .iter()
            .map(|r| Row { title: r.title.into(), leads: true, trailing: None, divider: false, glyph: false, thumb: None, icon: None })
            .collect()
    }

    pub fn refresh_sub_items(&mut self) {
        let id = self.roots[self.active_root].id;
        let mut items = menu::submenu_items(id, self.settings.sound_effects, self.settings.screen_saver, self.settings.video_quality.label());
        if id == "music" && self.player.has_session() {
            items.insert(
                0,
                SubItem { id: "music_now_playing".into(), title: "Now Playing".into(), leads_to_menu: true, trailing_text: None, shuffle_glyph: false, top_divider: false },
            );
        }
        if id == "network" {
            for p in self.disc.peers() {
                let mut it = menu::sub(&format!("net:{}", p.addr), &p.name, true);
                it.trailing_text = Some(p.addr.clone());
                items.push(it);
            }
        }
        let keep = self.sub_items.get(self.sub_sel).map(|s| s.id.clone());
        self.sub_items = items;
        if let Some(k) = keep {
            if let Some(i) = self.sub_items.iter().position(|s| s.id == k) {
                self.sub_sel = i;
            }
        }
        self.sub_sel = self.sub_sel.min(self.sub_items.len().saturating_sub(1));
    }

    // ----- per-frame update -----------------------------------------------------------

    pub fn update(&mut self, now: f64, keys: &[Key], chars: &[char]) {
        let in_text = matches!(self.screen, Screen::TextInput { .. });
        let mapped: Vec<Key> = keys.iter().map(|&k| if k == Key::Backspace && !in_text { Key::Back } else { k }).collect();
        let keys = &mapped[..];
        if keys.contains(&Key::Quit) {
            self.quit = true;
            return;
        }
        if !chars.is_empty() {
            self.type_chars(chars);
        }
        if self.level == Level::Sub && self.roots[self.active_root].id == "network" && matches!(self.screen, Screen::Menu) && now - self.last_net_refresh > 1.0 {
            self.last_net_refresh = now;
            self.refresh_sub_items();
        }
        // Background work.
        self.tex.poll();
        if let Some(rx) = &self.lib.tracks_rx {
            if let Ok(t) = rx.try_recv() {
                self.lib.tracks = t;
                self.lib.tracks_rx = None;
            }
        }
        if let Some(rx) = &self.lib.albums_rx {
            if let Ok(a) = rx.try_recv() {
                self.lib.albums = a;
                self.lib.albums_rx = None;
            }
        }
        if let Some(third) = self.thirds.last_mut() {
            if let Some(rx) = &third.loading {
                if let Ok(res) = rx.try_recv() {
                    third.loading = None;
                    match res {
                        Ok(entries) => third.entries = entries,
                        Err(e) => third.entries = vec![Entry::new(e, false, Action::Noop)],
                    }
                    third.sel = 0;
                    self.views[2].reset();
                }
            }
        }
        if self.player.update(&self.audio) && matches!(self.screen, Screen::NowPlaying) && !self.player.has_session() {
            self.back_to_menu(now);
        }

        // YouTube category lists loading in the background.
        for slot in self.yt_cache.values_mut() {
            if let YtSlot::Loading(rx) = slot {
                match rx.try_recv() {
                    Ok(Ok(items)) => *slot = YtSlot::Ready(items),
                    Ok(Err(_)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => *slot = YtSlot::Failed,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
        }

        // Black-overlay transitions.
        if self.pending.is_some() && self.fade.is_done(now) && self.fade.target() >= 1.0 {
            if let Some(f) = self.pending.take() {
                f(self, now);
            }
            self.fade.go(now, 0.0, 0.22);
        }

        if !keys.is_empty() || !chars.is_empty() {
            self.last_input = now;
            if matches!(self.screen, Screen::Saver(_)) {
                // Any key wakes from the screen saver.
                self.exit_saver(now);
                return;
            }
        }
        // Screen saver after a minute of idleness at the menu.
        if self.settings.screen_saver
            && matches!(self.screen, Screen::Menu)
            && self.pending.is_none()
            && now - self.last_input > self.saver_idle
            && !self.player.has_session()
        {
            self.start_saver(now);
        }

        match self.screen {
            Screen::Menu => {
                for &k in keys {
                    self.menu_key(now, k);
                }
            }
            _ => {
                for &k in keys {
                    self.screen_key(now, k);
                }
                self.screen_update(now);
            }
        }
        if let Some((_, until)) = &self.toast {
            if now > *until {
                self.toast = None;
            }
        }
    }

    // ----- menu navigation --------------------------------------------------------------

    fn menu_key(&mut self, now: f64, key: Key) {
        if key == Key::Quit {
            self.quit = true;
            return;
        }
        if self.pending.is_some() || now < self.busy_until {
            return;
        }
        match self.level {
            Level::Root => match key {
                Key::Up => self.move_root(now, -1),
                Key::Down => self.move_root(now, 1),
                Key::Enter | Key::Right => self.enter_submenu(now),
                Key::Back | Key::Left => {}
                _ => {}
            },
            Level::Sub => match key {
                Key::Up => self.move_list(now, -1),
                Key::Down => self.move_list(now, 1),
                Key::Enter | Key::Right | Key::Space => self.activate_sub(now),
                Key::Back | Key::Left => {
                    self.sfx("Exit");
                    self.return_to_root(now);
                }
                _ => {}
            },
            Level::Third => match key {
                Key::Up => self.move_list(now, -1),
                Key::Down => self.move_list(now, 1),
                Key::Enter | Key::Right | Key::Space => self.activate_third(now),
                Key::Channel => self.open_video_channel(now),
                Key::Back | Key::Left => {
                    self.sfx("Exit");
                    self.leave_third(now);
                }
                _ => {}
            },
        }
    }

    pub fn is_sweeping(&self) -> bool {
        self.linear_sweep
    }

    fn move_root(&mut self, now: f64, d: i32) {
        let n = self.roots.len() as i32;
        let next = self.sel as i32 + d;
        if next < 0 || next >= n {
            self.sfx("Limit");
            return;
        }
        self.sfx("SelectionChange");
        self.sel = next as usize;
        let dur = SELECTION_ANIM * 0.85;
        if self.linear_sweep {
            self.phase.go_with(now, self.sel as f32, 0.16 * 1.18, crate::anim::Curve::Linear);
        } else {
            self.phase.go(now, self.sel as f32, dur);
        }
    }

    fn move_list(&mut self, now: f64, d: i32) {
        let _ = now;
        let (sel, n) = match self.level {
            Level::Sub => (&mut self.sub_sel, self.sub_items.len()),
            Level::Third => match self.thirds.last_mut() {
                Some(t) => {
                    let n = if t.loading.is_some() { 1 } else { t.entries.len() };
                    (&mut t.sel, n)
                }
                None => return,
            },
            Level::Root => return,
        };
        let next = *sel as i32 + d;
        if next < 0 || next >= n as i32 {
            self.audio.play_sfx("Limit");
            return;
        }
        *sel = next as usize;
        self.audio.play_sfx("SelectionChange");
    }

    fn enter_submenu(&mut self, now: f64) {
        self.sfx("Selection");
        self.active_root = self.sel;
        self.refresh_sub_items();
        self.sub_sel = menu::default_submenu_index(self.roots[self.active_root].id);
        if self.roots[self.active_root].id == "music" && self.player.has_session() {
            self.sub_sel += 1;
        }
        self.sub_sel = self.sub_sel.min(self.sub_items.len().saturating_sub(1));
        self.header = self.roots[self.active_root].title.to_string();
        self.views[1].reset();
        self.level = Level::Sub;
        self.enter.go(now, 1.0, FLIGHT);
        self.root_alpha.go(now, 0.0, 0.18);
        self.sub_alpha.go_after(now, 0.35, 1.0, 0.25);
        self.busy_until = now + 0.45;
    }

    pub fn return_to_root(&mut self, now: f64) {
        self.thirds.clear();
        self.level = Level::Root;
        self.enter.go(now, 0.0, FLIGHT);
        self.sub_alpha.go(now, 0.0, 0.18);
        self.root_alpha.go_after(now, 0.3, 1.0, 0.25);
        self.header = "Marquee".into();
        self.views[0].reset();
        self.busy_until = now + 0.5;
    }

    pub fn leave_third(&mut self, now: f64) {
        self.swap(now, |app, _| {
            app.thirds.pop();
            if app.thirds.is_empty() {
                app.level = Level::Sub;
                app.header = app.roots[app.active_root].title.to_string();
                app.refresh_sub_items();
            } else {
                app.header = app.thirds.last().map(|t| t.title.clone()).unwrap_or_default();
            }
            app.views[2].reset();
        });
    }

    pub fn push_third(&mut self, now: f64, title: String, entries: Vec<Entry>) {
        self.push_third_with(now, title, entries, vec![]);
    }

    pub fn push_third_with(&mut self, now: f64, title: String, entries: Vec<Entry>, tracks: Vec<Track>) {
        self.swap(now, move |app, _| app.push_level_now(title, entries, tracks, None));
    }

    pub fn push_third_loading(&mut self, now: f64, title: String, rx: Receiver<Result<Vec<Entry>, String>>) {
        self.swap(now, move |app, _| app.push_level_now(title, vec![], vec![], Some(rx)));
    }

    /// Replaces the current third-level list in place (for folder navigation that should not stack).
    pub fn replace_third(&mut self, now: f64, title: String, entries: Vec<Entry>) {
        self.swap(now, move |app, _| {
            app.header = title.clone();
            if let Some(t) = app.thirds.last_mut() {
                t.title = title;
                t.entries = entries;
                t.sel = 0;
            }
            app.views[2].reset();
        });
    }

    pub fn push_level_now(&mut self, title: String, entries: Vec<Entry>, tracks: Vec<Track>, loading: Option<Receiver<Result<Vec<Entry>, String>>>) {
        self.header = title.clone();
        self.thirds.push(ThirdLevel { title, entries, sel: 0, tracks, loading });
        self.level = Level::Third;
        self.views[2].reset();
    }

    pub fn exit_saver(&mut self, now: f64) {
        self.ambient.stop();
        self.swap(now, |app, _| {
            app.screen = Screen::Menu;
        });
    }

    pub fn start_saver(&mut self, now: f64) {
        use ::rand::seq::SliceRandom;
        let mut all: Vec<PathBuf> = self.lib.albums.iter().flat_map(|a| a.photos.iter().map(|p| p.path.clone())).collect();
        all.shuffle(&mut ::rand::thread_rng());
        all.truncate(24);
        let s = crate::screens::Saver::new(&self.settings, all, now);
        self.set_screen(now, Screen::Saver(s));
        self.ambient.start_default(&self.audio);
    }

    pub fn save_settings(&mut self) {
        self.settings.save();
    }

    pub fn reload_media(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let folders = self.settings.folders(Kind::Music);
        std::thread::spawn(move || {
            let _ = tx.send(library::scan_music(&folders));
        });
        self.lib.tracks_rx = Some(rx);
        let (ptx, prx) = std::sync::mpsc::channel();
        let pf = self.settings.folders(Kind::Photos);
        std::thread::spawn(move || {
            let _ = ptx.send(library::scan_photos(&pf));
        });
        self.lib.albums_rx = Some(prx);
    }
}

/// Adwaita icon for a submenu row.
pub fn sub_icon(id: &str) -> Option<&'static str> {
    Some(match id {
        "movies_folder" => "folder-videos",
        "music_now_playing" => "media-playback-start",
        "music_shuffle_songs" => "media-playlist-shuffle",
        "music_music_videos" => "video-x-generic",
        "music_playlists" => "view-list-bullet",
        "music_artists" => "avatar-default",
        "music_albums" => "media-optical",
        "music_songs" => "audio-x-generic",
        "music_genres" => "audio-headphones",
        "music_composers" => "document-edit",
        "music_audiobooks" => "audio-volume-high",
        "photos_library" => "folder-pictures",
        "photos_last_12_months" => "x-office-calendar",
        "photos_last_import" => "document-open-recent",
        "settings_name" => "computer",
        "settings_swversion" => "dialog-information",
        "settings_screensaver" => "tv",
        "settings_soundeffects" => "audio-volume-high",
        "settings_quality" => "video-x-generic",
        "settings_media_folders" => "folder",
        "network_apps" => "view-app-grid",
        "network_add" => "list-add",
        id if id.starts_with("net:") => "network-server",
        _ => return None,
    })
}

/// Adwaita icon for a third-level entry, from what selecting it does.
pub fn icon_for(e: &Entry) -> Option<&'static str> {
    use crate::apps::Category;
    Some(match &e.action {
        Action::OpenDir(_) => "folder",
        Action::PlayVideo(_) => "video-x-generic",
        Action::Slideshow(_) => "image-x-generic",
        Action::PlayAt(_) => "audio-x-generic",
        Action::OpenTracks(..) => "folder-music",
        Action::Folders(_) => "folder",
        Action::Choose(..) => {
            if e.label.starts_with('‹') { "go-previous" } else { "folder" }
        }
        Action::AddFolder(..) => "object-select",
        Action::RemoveFolder(..) => "user-trash",
        Action::YtCategory(c) => match c {
            Category::Featured => "starred",
            Category::MostViewed => "tv",
            Category::MostRecent => "x-office-calendar",
            Category::TopRated => "object-select",
        },
        Action::YtHistory => "document-open-recent",
        Action::YtSubs => "avatar-default",
        Action::YtChannel(_) => "avatar-default",
        Action::YtToggleSub(_) => {
            if e.label == "Unsubscribe" { "non-starred" } else { "starred" }
        }
        Action::AppOpen(_) => "view-app-grid",
        Action::AppInstall(_) => "list-add",
        Action::AppRemove(_) => "user-trash",
        Action::AppPrompt(id) => {
            if id == "youtube_channels" { "emoji-people" } else { "system-search" }
        }
        Action::NetBrowse { p, kind, .. } => {
            if p.is_empty() {
                match kind.as_str() {
                    "music" => "folder-music",
                    "photos" => "folder-pictures",
                    _ => "folder-videos",
                }
            } else {
                "folder"
            }
        }
        Action::NetHost { .. } => "network-server",
        Action::AppsMenu => "view-app-grid",
        _ => return None,
    })
}
