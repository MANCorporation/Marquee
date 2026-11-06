//! Network browsing, Apps, and the text-entry screen.

use crate::app::*;
use crate::apps::{self, AppKind};
use crate::net;
use crate::texcache::Source;
use std::path::PathBuf;
use std::sync::mpsc::channel;

pub fn result_entries(items: &[apps::ResultItem]) -> Vec<Entry> {
    items
        .iter()
        .map(|it| {
            let mut e = Entry::new(it.title.clone(), false, Action::PlayPage(it.clone()));
            let mut tr = it.by.clone();
            if it.duration > 0.0 {
                tr = if tr.is_empty() { crate::gfx::fmt_time(it.duration) } else { format!("{} · {}", crate::gfx::fmt_time(it.duration), tr) };
            }
            if !tr.is_empty() {
                e = e.with_trailing(tr);
            }
            if let Some(t) = &it.thumb {
                e = e.with_thumb(format!("u:{t}"), Source::Url(t.clone()));
            }
            e
        })
        .collect()
}

pub fn channel_entries(chs: &[apps::ChannelRef]) -> Vec<Entry> {
    chs.iter()
        .map(|c| {
            let mut e = Entry::new(c.name.clone(), true, Action::YtChannel(c.clone()));
            if let Some(t) = &c.thumb {
                e = e.with_thumb(format!("u:{t}"), Source::Url(t.clone()));
            }
            e
        })
        .collect()
}

fn sub_entry(c: &apps::ChannelRef, subscribed: bool) -> Entry {
    Entry::new(if subscribed { "Unsubscribe" } else { "Subscribe" }, false, Action::YtToggleSub(c.clone())).with_trailing(if subscribed { "Subscribed" } else { "" })
}

fn stem(name: &str) -> String {
    std::path::Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or(name).to_string()
}

impl App {
    pub fn type_chars(&mut self, chars: &[char]) {
        if let Screen::TextInput { text, .. } = &mut self.screen {
            for &c in chars {
                if text.chars().count() < 200 {
                    text.push(c);
                }
            }
        }
    }

    pub fn open_text_input(&mut self, now: f64, prompt: &str, hint: &str, kind: TextKind) {
        self.sfx("Selection");
        self.set_screen(now, Screen::TextInput { prompt: prompt.into(), hint: hint.into(), text: String::new(), kind });
    }

    pub fn submit_text(&mut self, now: f64, kind: TextKind, text: String) {
        let text = text.trim().to_string();
        if text.is_empty() {
            self.back_to_menu(now);
            return;
        }
        match kind {
            TextKind::AddComputer => {
                self.disc.add_manual(&text);
                self.swap(now, |app, _| {
                    app.screen = Screen::Menu;
                    app.refresh_sub_items();
                });
            }
            TextKind::ChannelSearch => {
                if apps::ytdlp().is_none() {
                    return self.show_error(now, "YouTube", "YouTube needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp).");
                }
                let (tx, rx) = channel();
                let q = text.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(apps::list_channels(&q).map(|c| channel_entries(&c)));
                });
                let title = format!("YouTube › Channels: {text}");
                self.swap(now, move |app, _| {
                    app.screen = Screen::Menu;
                    app.push_level_now(title, vec![], vec![], Some(rx));
                });
            }
            TextKind::OpenLink => self.play_page(now, apps::ResultItem { title: text.clone(), url: text, by: String::new(), duration: 0.0, thumb: None, channel_url: None }),
            TextKind::Search(app_id) => {
                let Some(def) = apps::def(&app_id) else { return self.back_to_menu(now) };
                let AppKind::Search(prefix) = def.kind else { return self.back_to_menu(now) };
                if apps::ytdlp().is_none() {
                    return self.show_error(now, def.name, "This app needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp).");
                }
                let (tx, rx) = channel();
                let q = text.clone();
                std::thread::spawn(move || {
                    let res = apps::search(prefix, &q).map(|items| result_entries(&items));
                    let _ = tx.send(res);
                });
                let title = format!("{} › {}", def.name, text);
                self.swap(now, move |app, _| {
                    app.screen = Screen::Menu;
                    app.push_level_now(title, vec![], vec![], Some(rx));
                });
            }
        }
    }

    pub fn show_error(&mut self, now: f64, title: &str, msg: &str) {
        self.sfx("Limit");
        self.set_screen(now, Screen::Error { title: title.into(), message: msg.into() });
    }

    pub fn play_page(&mut self, now: f64, item: apps::ResultItem) {
        if !crate::ffmpeg::available() {
            return self.show_error(now, "Video", "Playing video needs FFmpeg.\nInstall it (brew install ffmpeg / apt install ffmpeg).");
        }
        if apps::ytdlp().is_none() {
            return self.show_error(now, "Video", "This needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp).");
        }
        self.sfx("Selection");
        self.ambient.stop();
        if self.player.has_session() {
            self.player.stop();
        }
        let (url, title, duration) = (item.url.clone(), item.title.clone(), item.duration);
        if url.contains("youtube.com") || url.contains("youtu.be") {
            self.settings.history.retain(|h| h.url != item.url);
            self.settings.history.insert(0, item);
            self.settings.history.truncate(50);
            self.save_settings();
        }
        let v = crate::videoplayer::VideoPlayback::open_resolving(url, title, duration, self.settings.video_quality);
        self.set_screen(now, Screen::Video(Box::new(v)));
    }

    pub fn net_host(&mut self, now: f64, addr: String, name: String) {
        self.sfx("Selection");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let res = net::libraries(&addr).map(|libs| {
                if libs.is_empty() {
                    return vec![Entry::new("No libraries are shared on this computer", false, Action::Noop)];
                }
                libs.into_iter()
                    .map(|l| {
                        let label = match l.kind.as_str() {
                            "music" => "Music",
                            "photos" => "Photos",
                            _ => "Movies",
                        };
                        Entry::new(l.name.clone(), true, Action::NetBrowse { addr: addr.clone(), lib: l.id, kind: l.kind.clone(), p: String::new(), title: l.name }).with_trailing(label)
                    })
                    .collect()
            });
            let _ = tx.send(res);
        });
        self.push_third_loading(now, name, rx);
    }

    pub fn net_browse(&mut self, now: f64, addr: String, lib: String, kind: String, p: String, title: String) {
        self.sfx("Selection");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let res = net::list(&addr, &lib, &p).map(|items| {
                if items.is_empty() {
                    return vec![Entry::new("Empty", false, Action::Noop)];
                }
                let photo_urls: Vec<PathBuf> = items.iter().filter(|e| !e.dir).map(|e| PathBuf::from(net::media_url(&addr, &lib, &e.p))).collect();
                let mut photo_i = 0;
                items
                    .into_iter()
                    .map(|e| {
                        if e.dir {
                            return Entry::new(e.name.clone(), true, Action::NetBrowse { addr: addr.clone(), lib: lib.clone(), kind: kind.clone(), p: e.p, title: e.name });
                        }
                        let url = net::media_url(&addr, &lib, &e.p);
                        match kind.as_str() {
                            "photos" => {
                                let mut rotated = photo_urls.clone();
                                rotated.rotate_left(photo_i.min(photo_urls.len().saturating_sub(1)));
                                photo_i += 1;
                                Entry::new(stem(&e.name), false, Action::Slideshow(rotated)).with_thumb(format!("u:{url}"), Source::Url(url))
                            }
                            "music" => Entry::new(stem(&e.name), false, Action::PlayVideo(PathBuf::from(url))),
                            _ => Entry::new(stem(&e.name), false, Action::PlayVideo(PathBuf::from(url.clone()))).with_thumb(format!("t:{url}"), Source::Thumb(url)),
                        }
                    })
                    .collect()
            });
            let _ = tx.send(res);
        });
        self.push_third_loading(now, title, rx);
    }

    /// Starts loading the browse lists in the background so the menu can preview them.
    pub fn yt_prefetch(&mut self) {
        if apps::ytdlp().is_none() {
            return;
        }
        for c in apps::Category::ALL {
            let needs = !matches!(self.yt_cache.get(&c), Some(YtSlot::Ready(_)) | Some(YtSlot::Loading(_)));
            if needs {
                let (tx, rx) = channel();
                std::thread::spawn(move || {
                    let _ = tx.send(apps::list(&c.url(), 12));
                });
                self.yt_cache.insert(c, YtSlot::Loading(rx));
            }
        }
    }

    pub fn yt_category(&mut self, now: f64, c: apps::Category) {
        self.sfx("Selection");
        let title = format!("YouTube › {}", c.label());
        if apps::ytdlp().is_none() {
            return self.show_error(now, "YouTube", "YouTube needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp).");
        }
        if let Some(YtSlot::Ready(items)) = self.yt_cache.get(&c) {
            let entries = result_entries(items);
            return self.push_third(now, title, entries);
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(apps::list(&c.url(), 12).map(|items| result_entries(&items)));
        });
        self.push_third_loading(now, title, rx);
    }

    pub fn yt_subs(&mut self, now: f64) {
        self.sfx("Selection");
        let entries = if self.settings.subs.is_empty() {
            vec![Entry::new("No subscriptions yet", false, Action::Noop), Entry::new("Open a channel and choose Subscribe", false, Action::Noop)]
        } else {
            channel_entries(&self.settings.subs.clone())
        };
        self.push_third(now, "YouTube › Subscriptions".into(), entries);
    }

    /// A channel page: Subscribe/Unsubscribe on top, then its videos.
    pub fn yt_channel(&mut self, now: f64, ch: apps::ChannelRef) {
        self.sfx("Selection");
        if apps::ytdlp().is_none() {
            return self.show_error(now, "YouTube", "YouTube needs yt-dlp.\nInstall it (brew install yt-dlp / pipx install yt-dlp).");
        }
        let subscribed = self.settings.subs.iter().any(|c| c.url == ch.url);
        let (tx, rx) = channel();
        let c2 = ch.clone();
        std::thread::spawn(move || {
            let res = apps::channel_videos(&c2.url).map(|items| {
                let mut v = vec![sub_entry(&c2, subscribed)];
                v.extend(result_entries(&items));
                v
            });
            let _ = tx.send(res);
        });
        self.push_third_loading(now, format!("YouTube › {}", ch.name), rx);
    }

    pub fn yt_toggle_sub(&mut self, ch: apps::ChannelRef) {
        self.sfx("Selection");
        if let Some(i) = self.settings.subs.iter().position(|c| c.url == ch.url) {
            self.settings.subs.remove(i);
        } else {
            self.settings.subs.push(ch.clone());
        }
        self.save_settings();
        let now_subscribed = self.settings.subs.iter().any(|c| c.url == ch.url);
        if let Some(t) = self.thirds.last_mut() {
            if let Some(first) = t.entries.first_mut() {
                if matches!(first.action, Action::YtToggleSub(_)) {
                    *first = sub_entry(&ch, now_subscribed);
                }
            }
        }
    }

    /// Key C: open the channel of the highlighted video.
    pub fn open_video_channel(&mut self, now: f64) {
        let item = self.thirds.last().and_then(|t| t.entries.get(t.sel)).and_then(|e| match &e.action {
            Action::PlayPage(it) => Some(it.clone()),
            _ => None,
        });
        match item.and_then(|it| it.channel_url.clone().map(|u| (u, it.by.clone(), it.thumb.clone()))) {
            Some((url, name, thumb)) => self.yt_channel(now, apps::ChannelRef { name: if name.is_empty() { "Channel".into() } else { name }, url, thumb: None }),
            None => self.sfx("Limit"),
        }
    }

    pub fn yt_history(&mut self, now: f64) {
        self.sfx("Selection");
        let entries = if self.settings.history.is_empty() {
            vec![Entry::new("No videos watched yet", false, Action::Noop)]
        } else {
            result_entries(&self.settings.history)
        };
        self.push_third(now, "YouTube › History".into(), entries);
    }

    pub fn apps_entries(&self) -> Vec<Entry> {
        let mut v = vec![];
        for d in apps::catalog() {
            if self.settings.installed_apps.iter().any(|i| i == d.id) {
                v.push(Entry::new(d.name, true, Action::AppOpen(d.id.into())).with_trailing(d.blurb));
            }
        }
        for d in apps::catalog() {
            if !self.settings.installed_apps.iter().any(|i| i == d.id) {
                v.push(Entry::new(format!("Get {}", d.name), false, Action::AppInstall(d.id.into())).with_trailing("Install"));
            }
        }
        v
    }

    pub fn apps_menu(&mut self, now: f64) {
        self.sfx("Selection");
        let e = self.apps_entries();
        self.push_third(now, "Apps".into(), e);
    }

    pub fn app_open(&mut self, now: f64, id: &str) {
        let Some(def) = apps::def(id) else { return };
        if id == "youtube" {
            self.sfx("Selection");
            self.yt_prefetch();
            let mut entries: Vec<Entry> = apps::Category::ALL.iter().map(|&c| Entry::new(c.label(), true, Action::YtCategory(c))).collect();
            entries.push(Entry::new("Subscriptions", true, Action::YtSubs));
            entries.push(Entry::new("History", true, Action::YtHistory));
            entries.push(Entry::new("Search", true, Action::AppPrompt("youtube".into())));
            entries.push(Entry::new("Search Channels", true, Action::AppPrompt("youtube_channels".into())));
            entries.push(Entry::new("Remove App", false, Action::AppRemove("youtube".into())));
            self.push_third(now, "YouTube".into(), entries);
            return;
        }
        match def.kind {
            AppKind::OpenLink => self.app_prompt(now, id),
            AppKind::Search(_) => {
                self.sfx("Selection");
                let entries = vec![
                    Entry::new(format!("Search {}…", def.name), false, Action::AppPrompt(id.into())),
                    Entry::new("Remove App", false, Action::AppRemove(id.into())),
                ];
                self.push_third(now, def.name.into(), entries);
            }
        }
    }

    pub fn app_prompt(&mut self, now: f64, id: &str) {
        if id == "youtube_channels" {
            return self.open_text_input(now, "Search YouTube Channels", "Type a channel name, then press Enter", TextKind::ChannelSearch);
        }
        let Some(def) = apps::def(id) else { return };
        match def.kind {
            AppKind::OpenLink => self.open_text_input(now, "Open Link", "Paste a video link, then press Enter", TextKind::OpenLink),
            AppKind::Search(_) => self.open_text_input(now, &format!("Search {}", def.name), "Type your search, then press Enter", TextKind::Search(id.into())),
        }
    }

    pub fn app_install(&mut self, id: &str) {
        self.sfx("Selection");
        if !self.settings.installed_apps.iter().any(|i| i == id) {
            self.settings.installed_apps.push(id.to_string());
            self.save_settings();
        }
        let e = self.apps_entries();
        if let Some(t) = self.thirds.last_mut() {
            t.entries = e;
        }
    }

    pub fn app_remove(&mut self, now: f64, id: &str) {
        self.sfx("Exit");
        self.settings.installed_apps.retain(|i| i != id);
        self.save_settings();
        let e = self.apps_entries();
        self.swap(now, move |app, _| {
            app.thirds.pop();
            if let Some(t) = app.thirds.last_mut() {
                t.entries = e;
                t.sel = 0;
                app.header = t.title.clone();
            }
            app.views[2].reset();
        });
    }

    pub fn draw_text_input(&self, prompt: &str, hint: &str, text: &str, now: f64) {
        use crate::gfx::*;
        use macroquad::prelude::*;
        let g = &self.g;
        g.text_centered(prompt, SCENE_W / 2.0, 330.0, 58.0, true, WHITE);
        let (x, y, w, h) = (360.0, 440.0, 1200.0, 96.0);
        draw_rectangle(x, y, w, h, Color::new(0.1, 0.08, 0.22, 0.9));
        draw_rectangle_lines(x, y, w, h, 3.0, Color::new(0.65, 0.5, 1.0, 0.9));
        g.sym.draw("system-search", x + 52.0, y + h / 2.0, 44.0, white(0.6));
        let shown = g.fit(&text.chars().rev().take(60).collect::<String>().chars().rev().collect::<String>(), w - 130.0, 44.0, false);
        g.text_mid(&shown, x + 96.0, y + h / 2.0, 44.0, false, WHITE);
        if (now * 2.0) as i64 % 2 == 0 {
            let cx = x + 96.0 + g.text_width(&shown, 44.0, false) + 4.0;
            draw_rectangle(cx, y + 24.0, 3.0, h - 48.0, WHITE);
        }
        g.text_centered(hint, SCENE_W / 2.0, 620.0, 30.0, false, white(0.55));
        g.text_centered("Enter to confirm  ·  Esc to cancel", SCENE_W / 2.0, 680.0, 26.0, false, white(0.4));
    }
}
