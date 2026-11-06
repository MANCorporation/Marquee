//! Full-screen scenes: Now Playing, photo slideshow, screen saver, error and movie hand-off.

use crate::app::*;
use crate::gfx::*;
use crate::settings::Settings;
use crate::texcache::Source;
use macroquad::prelude::*;
use std::path::PathBuf;

fn photo_src(p: &PathBuf) -> Source {
    let s = p.display().to_string();
    if s.starts_with("http") { Source::Url(s) } else { Source::File(p.clone()) }
}

pub const PHOTO_SECS: f64 = 3.0;
pub const CROSSFADE_SECS: f64 = 1.0;

// ---------------------------------------------------------------- slideshow

pub struct Slideshow {
    pub photos: Vec<PathBuf>,
    pub index: usize,
    pub prev: Option<usize>,
    pub shown_at: f64,
    pub paused: bool,
    pub paused_at: f64,
    seeds: Vec<(f32, f32, f32)>,
}

impl Slideshow {
    pub fn new(photos: Vec<PathBuf>, now: f64) -> Slideshow {
        use ::rand::Rng;
        let mut rng = ::rand::thread_rng();
        let seeds = photos.iter().map(|_| (rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(0.0..1.0))).collect();
        Slideshow { photos, index: 0, prev: None, shown_at: now + 1.0, paused: false, paused_at: 0.0, seeds }
    }

    fn key(&self, i: usize) -> String {
        format!("s:{}", self.photos[i % self.photos.len()].display())
    }
}

// ------------------------------------------------------------- screen saver

struct Card {
    src: CardSrc,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    w: f32,
    tilt: f32,
    phase: f32,
}

#[derive(Clone)]
enum CardSrc {
    Gen(u32),
    User(PathBuf),
}

pub struct Saver {
    cards: Vec<Card>,
    start: f64,
}

impl Saver {
    /// `photos`: the user's own pictures to float around; generated artwork is used when there are none.
    pub fn new(_settings: &Settings, photos: Vec<PathBuf>, now: f64) -> Saver {
        use ::rand::Rng;
        let mut rng = ::rand::thread_rng();
        let cards = (0..8)
            .map(|i| Card {
                src: if photos.is_empty() { CardSrc::Gen(i as u32 * 7 + 3) } else { CardSrc::User(photos[rng.gen_range(0..photos.len())].clone()) },
                x: rng.gen_range(-200.0..SCENE_W),
                y: rng.gen_range(150.0..SCENE_H - 150.0),
                vx: rng.gen_range(14.0..38.0),
                vy: rng.gen_range(-6.0..6.0),
                w: rng.gen_range(360.0..620.0),
                tilt: rng.gen_range(-0.12..0.12),
                phase: rng.gen_range(0.0..6.28),
            })
            .collect();
        Saver { cards, start: now }
    }
}

// ------------------------------------------------------------------- App impl

impl App {
    pub fn screen_key(&mut self, now: f64, key: Key) {
        if self.pending.is_some() {
            return;
        }
        let mut screen = std::mem::replace(&mut self.screen, Screen::Menu);
        match &mut screen {
            Screen::Menu | Screen::Saver(_) => {}
            Screen::TextInput { text, kind, .. } => match key {
                Key::Enter => {
                    let (t, k) = (text.clone(), kind.clone());
                    self.submit_text(now, k, t);
                }
                Key::Back => {
                    self.sfx("Exit");
                    self.back_to_menu(now);
                }
                Key::Backspace => {
                    text.pop();
                }
                _ => {}
            },
            Screen::Error { .. } => {
                if matches!(key, Key::Back | Key::Enter | Key::Left | Key::Space) {
                    self.sfx("Exit");
                    self.back_to_menu(now);
                }
            }
            Screen::Video(v) => match key {
                Key::Back => {
                    v.stop();
                    self.sfx("Exit");
                    self.back_to_menu(now);
                }
                Key::Space | Key::Enter => v.toggle_pause(now),
                Key::Left => v.seek_by(now, -10.0),
                Key::Right => v.seek_by(now, 10.0),
                Key::Up => v.seek_by(now, 60.0),
                Key::Down => v.seek_by(now, -60.0),
                Key::Quit | Key::Backspace | Key::Channel => {}
            },
            Screen::External { child, .. } => {
                if key == Key::Back {
                    if let Some(c) = child.as_mut() {
                        let _ = c.kill();
                        let _ = c.wait();
                    }
                    *child = None;
                    self.back_to_menu(now);
                }
            }
            Screen::NowPlaying => match key {
                Key::Back => {
                    self.sfx("Exit");
                    self.back_to_menu(now);
                }
                Key::Space | Key::Enter => self.player.toggle_pause(),
                Key::Up => self.player.prev(&self.audio),
                Key::Down => self.player.next(&self.audio),
                Key::Left => self.player.seek_by(-10.0),
                Key::Right => self.player.seek_by(10.0),
                _ => {}
            },
            Screen::Slideshow(s) => match key {
                Key::Back => {
                    self.sfx("Exit");
                    self.ambient.stop();
                    self.back_to_menu(now);
                }
                Key::Space | Key::Enter => {
                    if s.paused {
                        s.shown_at += now - s.paused_at;
                    } else {
                        s.paused_at = now;
                    }
                    s.paused = !s.paused;
                }
                Key::Right | Key::Down => {
                    s.prev = Some(s.index);
                    s.index = (s.index + 1) % s.photos.len();
                    s.shown_at = now;
                }
                Key::Left | Key::Up => {
                    s.prev = Some(s.index);
                    s.index = (s.index + s.photos.len() - 1) % s.photos.len();
                    s.shown_at = now;
                }
                _ => {}
            },
        }
        if matches!(self.screen, Screen::Menu) {
            self.screen = screen;
        }
    }

    pub fn screen_update(&mut self, now: f64) {
        let mut screen = std::mem::replace(&mut self.screen, Screen::Menu);
        let mut leave = false;
        match &mut screen {
            Screen::Slideshow(s) => {
                let cur = s.key(s.index);
                let src = photo_src(&s.photos[s.index]);
                let loaded = matches!(self.tex.get(&cur, src, 2560), Some(_));
                // Warm the next photo.
                let nxt = (s.index + 1) % s.photos.len();
                let nk = s.key(nxt);
                let _ = self.tex.get(&nk, photo_src(&s.photos[nxt]), 2560);
                if !loaded {
                    s.shown_at = now.max(s.shown_at);
                } else if !s.paused && now - s.shown_at >= PHOTO_SECS && now >= s.shown_at {
                    s.prev = Some(s.index);
                    s.index = nxt;
                    s.shown_at = now;
                }
            }
            Screen::Video(v) => {
                v.update(now, &self.audio);
                if let Some(e) = v.error.take() {
                    v.stop();
                    self.pending_error = Some(("Video".into(), e));
                } else if v.finished {
                    v.stop();
                    leave = true;
                }
            }
            Screen::External { child, started, .. } => {
                if let Some(c) = child.as_mut() {
                    if let Ok(Some(_status)) = c.try_wait() {
                        if now - *started > 0.5 {
                            *child = None;
                            leave = true;
                        }
                    }
                }
            }
            _ => {}
        }
        if matches!(self.screen, Screen::Menu) {
            self.screen = screen;
        }
        if let Some((t, m)) = self.pending_error.take() {
            if self.pending.is_none() {
                self.sfx("Limit");
                self.set_screen(now, Screen::Error { title: t, message: m });
            } else {
                self.pending_error = Some((t, m));
            }
        }
        if leave && self.pending.is_none() {
            self.back_to_menu(now);
        }
    }

    // ----- drawing -------------------------------------------------------------------

    pub fn draw_screen(&mut self, now: f64) {
        let mut screen = std::mem::replace(&mut self.screen, Screen::Menu);
        match &mut screen {
            Screen::Menu => {
                self.screen = screen;
                self.draw_menu(now);
                return;
            }
            Screen::NowPlaying => self.draw_now_playing(now),
            Screen::Slideshow(s) => self.draw_slideshow(now, s),
            Screen::Saver(s) => self.draw_saver(now, s),
            Screen::Error { title, message } => self.draw_error(title, message),
            Screen::TextInput { prompt, hint, text, .. } => self.draw_text_input(prompt, hint, text, now),
            Screen::Video(v) => {
                v.draw(now, &self.g);
            }
            Screen::External { player, title, .. } => {
                self.g.text_centered(&format!("Playing “{title}”"), SCENE_W / 2.0, 480.0, 44.0, true, white(0.9));
                self.g.text_centered(&format!("in {player} — press Esc here to stop"), SCENE_W / 2.0, 550.0, 30.0, false, white(0.5));
            }
        }
        if matches!(self.screen, Screen::Menu) {
            self.screen = screen;
        }
    }

    fn draw_error(&mut self, title: &str, message: &str) {
        self.g.sym.draw("dialog-warning", SCENE_W / 2.0, 340.0, 180.0, Color::new(1.0, 0.8, 0.2, 1.0));
        self.g.text_centered(title, SCENE_W / 2.0, 520.0, 56.0, true, WHITE);
        for (i, line) in message.lines().enumerate() {
            self.g.text_centered(line, SCENE_W / 2.0, 600.0 + i as f32 * 46.0, 34.0, false, white(0.7));
        }
    }

    fn draw_slideshow(&mut self, now: f64, s: &mut Slideshow) {
        let draw_one = |app: &mut App, i: usize, alpha: f32, started: f64, s: &Slideshow| {
            let key = s.key(i);
            let src = photo_src(&s.photos[i % s.photos.len()]);
            if let Some(Some(tex)) = app.tex.get(&key, src, 2560) {
                let seed = s.seeds[i % s.seeds.len()];
                let p = (((now - started) as f32) / (PHOTO_SECS + CROSSFADE_SECS * 2.0) as f32).clamp(0.0, 1.0);
                let zoom = 1.0 + 0.07 * if seed.2 > 0.5 { p } else { 1.0 - p };
                let (tw, th) = (tex.width().max(1.0), tex.height().max(1.0));
                let fit = (SCENE_W / tw).min(SCENE_H / th) * zoom;
                let (w, h) = (tw * fit, th * fit);
                let pan = vec2(seed.0 * 30.0 * (p - 0.5), seed.1 * 20.0 * (p - 0.5));
                draw_tex(tex, (SCENE_W - w) / 2.0 + pan.x, (SCENE_H - h) / 2.0 + pan.y, w, h, white(alpha));
            }
        };
        let t = (((now - s.shown_at) / CROSSFADE_SECS).clamp(0.0, 1.0)) as f32;
        if let Some(prev) = s.prev {
            if t < 1.0 {
                draw_one(self, prev, 1.0, s.shown_at - PHOTO_SECS, s);
            }
        }
        draw_one(self, s.index, if s.prev.is_some() { t } else { ((now - s.shown_at + 1.0).clamp(0.0, 1.0)) as f32 }, s.shown_at, s);
        if s.paused {
            self.g.sym.draw("media-playback-pause", SCENE_W - 100.0, 86.0, 56.0, white(0.85));
        }
    }

    fn draw_saver(&mut self, now: f64, s: &mut Saver) {
        let t = (now - s.start) as f32;
        let fade_in = (t / 2.0).clamp(0.0, 1.0);
        for c in &s.cards {
            let span = SCENE_W + 1000.0;
            let x = ((c.x + 500.0 + c.vx * t).rem_euclid(span)) - 500.0;
            let y = c.y + c.vy * t + (t * 0.25 + c.phase).sin() * 40.0;
            let (key, src) = match &c.src {
                CardSrc::Gen(n) => (format!("gen:{n}"), Source::Generated(*n)),
                CardSrc::User(p) => (format!("saver:{}", p.display()), Source::File(p.clone())),
            };
            if let Some(Some(tex)) = self.tex.get(&key, src, 1024) {
                let aspect = tex.width() / tex.height().max(1.0);
                let (w, h) = (c.w, c.w / aspect);
                let edge = ((x / 300.0).clamp(0.0, 1.0)).min(((SCENE_W - x) / 300.0).clamp(0.0, 1.0));
                let a = fade_in * edge.max(0.15);
                draw_rectangle_ex(
                    x - (w + 24.0) / 2.0, y - (h + 24.0) / 2.0, w + 24.0, h + 24.0,
                    DrawRectangleParams { offset: vec2(0.5, 0.5), rotation: c.tilt, color: Color::new(0.0, 0.0, 0.0, 0.5 * a) },
                );
                draw_texture_ex(
                    tex, x - w / 2.0, y - h / 2.0, white(a),
                    DrawTextureParams { dest_size: Some(vec2(w, h)), rotation: c.tilt, ..Default::default() },
                );
            }
        }
    }

    fn draw_now_playing(&mut self, now: f64) {
        let Some(track) = self.player.current().cloned() else { return };
        // The layout flips left-to-right every 18 s to protect displays from burn-in.
        let period = 18.0;
        let flipped = ((now / period) as u64) % 2 == 1;
        let since = (now % period) as f32;
        let a = (since / 0.6).clamp(0.0, 1.0);
        let mx = |x: f32| if flipped { SCENE_W - x } else { x };

        // Cover art with its reflection.
        let key = format!("c:{}", track.path.display());
        let (ax, ay, aw, ah) = (1200.0f32, 290.0f32, 520.0f32, 520.0f32);
        let art_x = if flipped { SCENE_W - ax - aw } else { ax };
        let tex = match self.tex.get(&key, Source::Cover(track.path.clone()), 1024) {
            Some(Some(t)) => Some(t.clone()),
            _ => None,
        };
        let tex = tex.unwrap_or_else(|| self.g.icons["music"].sharp.clone());
        let r = draw_tex_fit(&tex, art_x, ay, aw, ah, a);
        let rc = white(0.38 * a);
        let z = white(0.0);
        let rh = r.h * 0.6;
        quad_mesh(&tex, [(r.x, r.y + r.h, 0.0, 1.0, rc), (r.x + r.w, r.y + r.h, 1.0, 1.0, rc), (r.x + r.w, r.y + r.h + rh, 1.0, 0.4, z), (r.x, r.y + r.h + rh, 0.0, 0.4, z)]);

        // Text block, right-aligned against x = 1128 (mirrored when flipped).
        let right = 1128.0f32;
        let put = |app: &App, text: &str, y: f32, size: f32, bold: bool, c: Color| {
            let text = app.g.fit(text, 880.0, size, bold);
            if flipped {
                app.g.text_mid(&text, mx(right), y, size, bold, c);
            } else {
                app.g.text_right(&text, right, y, size, bold, c);
            }
        };
        put(self, &track.title, 598.0, 46.0, true, white(a));
        put(self, &track.artist, 653.0, 42.0, false, white(0.92 * a));
        put(self, &track.album, 708.0, 42.0, false, white(0.62 * a));
        if self.player.queue.len() > 1 {
            put(self, &format!("{} of {}", self.player.index + 1, self.player.queue.len()), 760.0, 28.0, false, white(0.45 * a));
        }

        if self.player.shuffled {
            let sx = if flipped { SCENE_W - 186.0 } else { 186.0 };
            self.g.sym.draw("media-playlist-shuffle", sx, 768.0, 32.0, white(0.6 * a));
        }

        // Progress.
        let pos = self.player.position();
        let dur = self.player.duration();
        let (bx0, bx1, by) = (247.0f32, 1033.0f32, 797.0f32);
        let (x0, x1) = if flipped { (SCENE_W - bx1, SCENE_W - bx0) } else { (bx0, bx1) };
        draw_rectangle_lines(x0, by, x1 - x0, 22.0, 2.5, white(0.9 * a));
        let frac = if dur > 0.0 { (pos / dur).clamp(0.0, 1.0) } else { 0.0 };
        draw_rectangle(x0 + 2.0, by + 2.0, (x1 - x0 - 4.0) * frac, 18.0, white(0.45 * a));
        let mxk = x0 + 2.0 + (x1 - x0 - 4.0) * frac;
        draw_poly(mxk, by + 11.0, 4, 6.0, 45.0, white(a));
        let (l, rt) = if flipped { (SCENE_W - 170.0, 792.0) } else { (170.0, 1128.0) };
        let elapsed = fmt_time(pos);
        let remaining = format!("-{}", fmt_time((dur - pos).max(0.0)));
        if flipped {
            self.g.text_right(&elapsed, l, 807.0, 28.0, true, white(a));
            self.g.text_mid(&remaining, SCENE_W - rt - 336.0, 807.0, 28.0, true, white(a));
        } else {
            self.g.text_mid(&elapsed, l, 807.0, 28.0, true, white(a));
            self.g.text_right(&remaining, rt, 807.0, 28.0, true, white(a));
        }
        if self.player.is_paused() {
            let px = if flipped { SCENE_W - 200.0 } else { 200.0 };
            self.g.sym.draw("media-playback-pause", px, 716.0, 44.0, white(0.85 * a));
        }
    }
}
