//! In-app video playback, streamed live: nothing is downloaded first.
//!
//! One FFmpeg process reads the source (a local file, a network share, or the direct video/audio streams of a
//! web video), decodes it (hardware where available), and writes raw RGBA frames to its stdout and PCM audio to a
//! local TCP socket. The audio clock decides which frame is on screen, so picture and sound stay in sync.
//! Seeking restarts FFmpeg at the new position, which reads only what it needs, so 4K and 8-hour videos work.

use crate::apps::{self, Input, Stream};
use crate::audio::Audio;
use crate::ffmpeg;
use crate::settings::Quality;
use macroquad::prelude::*;
use rodio::buffer::SamplesBuffer;
use rodio::Sink;
use std::io::Read;
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CHUNK_SECS: f32 = 0.5;

struct Frame {
    pts: f32,
    data: Vec<u8>,
}

#[derive(Clone)]
enum Source {
    /// A file path or an http(s) URL FFmpeg can open directly.
    Direct(String),
    /// A web page (YouTube, SoundCloud…) that yt-dlp turns into streams.
    Page(String),
}

struct Pipeline {
    child: Child,
    frames: Option<Receiver<Frame>>,
    first: Option<Frame>,
    sink: Option<Arc<Sink>>,
    appended: Arc<AtomicUsize>,
    audio_present: Arc<AtomicBool>,
    audio_eof: Arc<AtomicBool>,
    video_eof: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    dims: (u32, u32),
    fps: f32,
}

impl Pipeline {
    fn kill(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(s) = self.sink.take() {
            s.stop();
        }
    }
}

type Starting = Receiver<Result<(Stream, Pipeline), String>>;

pub struct VideoPlayback {
    pub title: String,
    source: Source,
    quality: Quality,
    duration_hint: f32,
    pub error: Option<String>,
    pub status: Arc<Mutex<String>>,
    stream: Option<Stream>,
    starting: Option<Starting>,
    starting_at: f32,
    pipe: Option<Pipeline>,
    pub tex: Option<Texture2D>,
    tex_dims: (u32, u32),
    base: f32,
    wall_acc: f32,
    wall_start: Option<Instant>,
    pub paused: bool,
    pub controls_until: f64,
    seek_target: Option<(f32, f64)>,
    pub finished: bool,
    restarts: u32,
    last_restart: Option<Instant>,
    audio_only: bool,
}

fn even(v: f32) -> u32 {
    ((v.round() as u32) / 2 * 2).max(2)
}

impl VideoPlayback {
    pub fn open(src: String, title: String, quality: Quality) -> VideoPlayback {
        Self::new(Source::Direct(src), title, 0.0, quality)
    }

    /// Opens a web page link (YouTube etc.): yt-dlp finds the live streams.
    pub fn open_resolving(src: String, title: String, duration: f32, quality: Quality) -> VideoPlayback {
        Self::new(Source::Page(src), title, duration, quality)
    }

    fn new(source: Source, title: String, duration_hint: f32, quality: Quality) -> VideoPlayback {
        VideoPlayback {
            title,
            source,
            quality,
            duration_hint,
            error: None,
            status: Arc::new(Mutex::new(String::new())),
            stream: None,
            starting: None,
            starting_at: 0.0,
            pipe: None,
            tex: None,
            tex_dims: (0, 0),
            base: 0.0,
            wall_acc: 0.0,
            wall_start: None,
            paused: false,
            controls_until: 0.0,
            seek_target: None,
            finished: false,
            restarts: 0,
            last_restart: None,
            audio_only: false,
        }
    }

    pub fn duration(&self) -> f32 {
        let d = self.stream.as_ref().map(|s| s.duration).unwrap_or(0.0);
        if d > 0.0 { d } else { self.duration_hint }
    }

    pub fn is_loading(&self) -> bool {
        self.starting.is_some() || self.pipe.is_none()
    }

    /// Current playback position in seconds.
    pub fn clock(&self) -> f32 {
        if let Some((t, _)) = self.seek_target {
            return t;
        }
        if let Some(p) = &self.pipe {
            if let (true, Some(s)) = (p.audio_present.load(Ordering::SeqCst), &p.sink) {
                // `get_pos` is relative to the queued chunk now playing; add the chunks already finished.
                let done = p.appended.load(Ordering::SeqCst).saturating_sub(s.len());
                return self.base + done as f32 * CHUNK_SECS + s.get_pos().as_secs_f32();
            }
        }
        self.base + self.wall_acc + self.wall_start.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0)
    }

    fn kill(&mut self) {
        if let Some(mut p) = self.pipe.take() {
            p.kill();
        }
        self.starting = None;
    }

    pub fn stop(&mut self) {
        self.kill();
    }

    /// Starts (or restarts) streaming at `at` seconds. Resolving and connecting happen on a worker thread.
    fn start_at(&mut self, audio: &Audio, at: f32, re_resolve: bool) {
        self.kill();
        self.finished = false;
        self.base = at;
        self.wall_acc = 0.0;
        self.wall_start = None;
        let sink = audio.new_sink().map(Arc::new);
        if self.paused {
            if let Some(s) = &sink {
                s.pause();
            }
        }
        let cached = if re_resolve { None } else { self.stream.clone() };
        let (source, quality, status) = (self.source.clone(), self.quality, self.status.clone());
        // Output size needs the window size, which only the main thread can read.
        let view = (screen_width() * screen_dpi_scale(), screen_height() * screen_dpi_scale());
        let probe_src = match &self.source {
            Source::Direct(s) => Some(s.clone()),
            _ => None,
        };
        let (tx, rx) = channel();
        self.starting_at = at;
        std::thread::spawn(move || {
            let res = (|| {
                let stream = match cached {
                    Some(s) => s,
                    None => match &source {
                        Source::Direct(path) => {
                            let p = ffmpeg::probe(path)?;
                            let inp = Input { url: path.clone(), headers: vec![] };
                            Stream {
                                video: p.has_video.then(|| inp.clone()),
                                audio: p.has_audio.then(|| inp.clone()),
                                width: p.width,
                                height: p.height,
                                fps: p.fps,
                                duration: p.duration,
                            }
                        }
                        Source::Page(url) => apps::resolve_stream(url, quality.stream_height(), &status)?,
                    },
                };
                let _ = probe_src;
                if let Ok(mut s) = status.lock() {
                    *s = "Buffering…".into();
                }
                let pipe = spawn_pipeline(&stream, at, quality, view, sink)?;
                Ok((stream, pipe))
            })();
            let _ = tx.send(res);
        });
        self.starting = Some(rx);
    }

    pub fn toggle_pause(&mut self, now: f64) {
        self.paused = !self.paused;
        self.controls_until = now + 3.0;
        let audio_clock = self.pipe.as_ref().map(|p| p.audio_present.load(Ordering::SeqCst)).unwrap_or(false);
        if let Some(s) = self.pipe.as_ref().and_then(|p| p.sink.as_ref()) {
            if self.paused { s.pause() } else { s.play() }
        }
        if !audio_clock {
            if self.paused {
                if let Some(t) = self.wall_start.take() {
                    self.wall_acc += t.elapsed().as_secs_f32();
                }
            } else {
                self.wall_start = Some(Instant::now());
            }
        }
    }

    /// Jumps by `delta` seconds; the stream restarts once the key presses settle.
    pub fn seek_by(&mut self, now: f64, delta: f32) {
        let dur = self.duration();
        let mut t = self.clock() + delta;
        t = if dur > 0.0 { t.clamp(0.0, (dur - 1.0).max(0.0)) } else { t.max(0.0) };
        self.seek_target = Some((t, now + 0.35));
        self.controls_until = now + 3.0;
    }

    pub fn update(&mut self, now: f64, audio: &Audio) {
        // First start.
        if self.pipe.is_none() && self.starting.is_none() && self.error.is_none() && !self.finished {
            self.controls_until = now + 3.0;
            self.start_at(audio, 0.0, false);
        }
        // A connection being set up.
        if let Some(rx) = &self.starting {
            match rx.try_recv() {
                Ok(Ok((stream, mut pipe))) => {
                    self.starting = None;
                    self.audio_only = stream.video.is_none();
                    self.stream = Some(stream);
                    if pipe.audio_present.load(Ordering::SeqCst) {
                        self.wall_start = None;
                    } else if !self.paused {
                        self.wall_start = Some(Instant::now());
                    }
                    // Show the first frame right away.
                    if let Some(f) = pipe.first.take() {
                        self.show(&f.data, pipe.dims);
                    }
                    self.pipe = Some(pipe);
                }
                Ok(Err(e)) => {
                    self.starting = None;
                    self.error = Some(e);
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.starting = None;
                    self.error = Some("Playback could not start.".into());
                    return;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
            if self.starting.is_some() {
                return;
            }
        }
        if let Some((t, at)) = self.seek_target {
            if now >= at {
                self.seek_target = None;
                self.start_at(audio, t, false);
            }
            return;
        }
        let clock = self.clock();
        if std::env::var_os("MARQUEE_DEBUG").is_some() && (now * 0.5) as i64 != ((now - 0.03) * 0.5) as i64 {
            if let Some(p) = &self.pipe {
                eprintln!("t={clock:.1} dims={:?} fps={} next_frame={:?} audio={}", p.dims, p.fps, p.first.as_ref().map(|f| f.pts), p.audio_present.load(Ordering::SeqCst));
            }
        }
        let mut latest: Option<(Vec<u8>, (u32, u32))> = None;
        let mut drained = true;
        if let Some(p) = self.pipe.as_mut() {
            // Show the newest frame that is due; older ones that are already late are dropped (not uploaded).
            if let Some(rx) = &p.frames {
                loop {
                    let next = match p.first.take() {
                        Some(f) => Some(f),
                        None => match rx.try_recv() {
                            Ok(f) => Some(f),
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                drained = false;
                                None
                            }
                            Err(_) => None,
                        },
                    };
                    match next {
                        Some(f) if f.pts <= clock + 0.02 => latest = Some((f.data, p.dims)),
                        Some(f) => {
                            p.first = Some(f);
                            drained = false;
                            break;
                        }
                        None => break,
                    }
                }
            }
        }
        if let Some((data, dims)) = latest {
            self.show(&data, dims);
        }
        // The end: both decoders hit end-of-file and everything they produced has been played.
        let mut ended = false;
        let mut died = false;
        if let Some(p) = self.pipe.as_mut() {
            let video_done = p.frames.is_none() || (p.video_eof.load(Ordering::SeqCst) && p.first.is_none() && drained);
            let audio_done = match &p.sink {
                None => true,
                Some(s) => !p.audio_present.load(Ordering::SeqCst) || (p.audio_eof.load(Ordering::SeqCst) && s.empty()),
            };
            if video_done && audio_done && !self.paused {
                let dur = self.stream.as_ref().map(|s| s.duration).unwrap_or(0.0).max(self.duration_hint);
                if dur > 0.0 && clock < dur - 4.0 {
                    died = true; // ended early: the connection dropped or a URL expired
                } else {
                    ended = true;
                }
            }
        }
        if died {
            if self.restarts < 4 && self.last_restart.map(|t| t.elapsed().as_secs() > 5).unwrap_or(true) {
                self.restarts += 1;
                self.last_restart = Some(Instant::now());
                let t = clock;
                let re = matches!(self.source, Source::Page(_));
                self.start_at(audio, t, re);
            } else {
                self.error = Some("The connection was lost.".into());
            }
        } else if ended {
            self.finished = true;
        }
    }

    fn show(&mut self, data: &[u8], dims: (u32, u32)) {
        let (w, h) = dims;
        match &self.tex {
            Some(t) if self.tex_dims == dims => t.update_from_bytes(w, h, data),
            _ => {
                let t = Texture2D::from_rgba8(w as u16, h as u16, data);
                t.set_filter(FilterMode::Linear);
                self.tex = Some(t);
                self.tex_dims = dims;
            }
        }
    }

    pub fn draw(&self, now: f64, font: &crate::gfx::Gfx) {
        use crate::gfx::*;
        if let Some(t) = &self.tex {
            draw_tex_fit(t, 0.0, 0.0, SCENE_W, SCENE_H, 1.0);
        } else if self.audio_only {
            let icon = &font.icons["music"].sharp;
            draw_tex_fit(icon, SCENE_W / 2.0 - 220.0, 220.0, 440.0, 440.0, 1.0);
            font.text_centered(&self.title, SCENE_W / 2.0, 760.0, 46.0, true, WHITE);
        }
        if self.is_loading() {
            let dots = ".".repeat(1 + ((now * 2.0) as usize % 3));
            let msg = self.status.lock().map(|s| s.clone()).unwrap_or_default();
            let msg = if msg.is_empty() { format!("Loading{dots}") } else { msg };
            if self.tex.is_none() {
                font.text_centered(&msg, SCENE_W / 2.0, SCENE_H / 2.0, 44.0, false, white(0.7));
                font.text_centered(&self.title, SCENE_W / 2.0, SCENE_H / 2.0 + 70.0, 30.0, false, white(0.4));
            } else {
                font.text_centered(&msg, SCENE_W / 2.0, 120.0, 34.0, false, white(0.85));
            }
        }
        if self.paused && !self.is_loading() {
            // A large play button while paused.
            draw_circle(SCENE_W / 2.0, SCENE_H / 2.0, 84.0, Color::new(0.0, 0.0, 0.0, 0.5));
            font.sym.draw("media-playback-start", SCENE_W / 2.0 + 4.0, SCENE_H / 2.0, 88.0, white(0.95));
        }
        let show = self.paused || now < self.controls_until || self.seek_target.is_some();
        if !show {
            return;
        }
        let (x0, x1, y) = (160.0f32, 1760.0f32, 962.0f32);
        let h = 44.0;
        draw_rectangle(x0, y, x1 - x0, h, Color::new(0.02, 0.03, 0.08, 0.82));
        draw_circle(x0, y + h / 2.0, h / 2.0, Color::new(0.02, 0.03, 0.08, 0.82));
        draw_circle(x1, y + h / 2.0, h / 2.0, Color::new(0.02, 0.03, 0.08, 0.82));
        let clock = self.clock();
        let dur = self.duration().max(0.001);
        let frac = (clock / dur).clamp(0.0, 1.0);
        let (bx0, bx1) = (x0 + 90.0, x1 - 150.0);
        draw_rectangle(bx0, y + 8.0, (bx1 - bx0) * frac, h - 16.0, Color::new(0.45, 0.8, 0.96, 1.0));
        draw_rectangle_lines(bx0, y + 8.0, bx1 - bx0, h - 16.0, 2.0, white(0.35));
        draw_poly(bx0 + (bx1 - bx0) * frac, y + h / 2.0, 4, 9.0, 45.0, white(1.0));
        font.sym.draw(if self.paused { "media-playback-start" } else { "media-playback-pause" }, x0 + 40.0, y + h / 2.0, 30.0, WHITE);
        font.text_right(&fmt_time((dur - clock).max(0.0)), x1 - 24.0, y + h / 2.0, 28.0, true, WHITE);
        font.text_centered(&fmt_time(clock), (bx0 + (bx1 - bx0) * frac).clamp(bx0 + 30.0, bx1 - 30.0), y + h + 24.0, 26.0, true, white(0.95));
        // Picture size being decoded.
        if let Some(p) = &self.pipe {
            let (w, hh) = p.dims;
            if w > 0 {
                font.text_right(&format!("{w}×{hh}  {:.0} fps", p.fps), x1, y - 22.0, 22.0, false, white(0.45));
            }
        }
    }
}

impl Drop for VideoPlayback {
    fn drop(&mut self) {
        self.kill();
    }
}

fn spawn_pipeline(stream: &Stream, at: f32, quality: Quality, _view: (f32, f32), sink: Option<Arc<Sink>>) -> Result<Pipeline, String> {
    let ffmpeg_bin = ffmpeg::tool("ffmpeg").ok_or("ffmpeg was not found. Install FFmpeg to play videos in the app.")?;
    let has_video = stream.video.is_some() && stream.width > 0;
    let has_audio = stream.audio.is_some();
    if !has_video && !has_audio {
        return Err("No playable audio or video was found.".into());
    }
    // (width, height) are computed on the main thread's view in practice; this keeps the worker self-contained.
    let (w, h) = if has_video { out_dims_from(stream.width, stream.height, quality, _view) } else { (0, 0) };
    let fps_cap = if (w as u64) * (h as u64) <= 1280 * 720 { 60.0 } else if quality == Quality::Low { 30.0 } else { 60.0 };
    let fps = if stream.fps > 1.0 { stream.fps.min(fps_cap) } else { 30.0 };

    let listener = if has_audio { Some(TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?) } else { None };
    let port = listener.as_ref().and_then(|l| l.local_addr().ok()).map(|a| a.port()).unwrap_or(0);

    let mut cmd = Command::new(ffmpeg_bin);
    cmd.args(["-v", "error", "-nostdin"]);
    let same_input = stream.video.as_ref().map(|v| &v.url) == stream.audio.as_ref().map(|a| &a.url) && has_video && has_audio;
    let mut inputs: Vec<&Input> = vec![];
    if let Some(v) = stream.video.as_ref().filter(|_| has_video) {
        inputs.push(v);
    }
    if let (Some(a), false) = (stream.audio.as_ref(), same_input) {
        inputs.push(a);
    }
    for (i, inp) in inputs.iter().enumerate() {
        if has_video && i == 0 {
            cmd.args(["-hwaccel", "auto"]);
        }
        if inp.url.starts_with("http") {
            cmd.args(["-reconnect", "1", "-reconnect_streamed", "1", "-reconnect_delay_max", "5"]);
            let mut hdr = String::new();
            for (k, v) in &inp.headers {
                if k.eq_ignore_ascii_case("user-agent") {
                    cmd.args(["-user_agent", v]);
                } else {
                    hdr.push_str(&format!("{k}: {v}\r\n"));
                }
            }
            if !hdr.is_empty() {
                cmd.args(["-headers", &hdr]);
            }
        }
        if at > 0.5 {
            cmd.args(["-ss", &format!("{at:.3}")]);
        }
        cmd.args(["-i", &inp.url]);
    }
    let audio_idx = if same_input || inputs.len() == 1 { 0 } else { 1 };
    if has_video {
        cmd.args(["-map", "0:v:0", "-an", "-sn", "-vf", &format!("fps={fps},scale={w}:{h}:flags=fast_bilinear"), "-pix_fmt", "rgba", "-f", "rawvideo", "pipe:1"]);
    }
    if has_audio {
        cmd.args(["-map", &format!("{audio_idx}:a:0"), "-vn", "-sn", "-ac", "2", "-ar", "44100", "-f", "f32le", &format!("tcp://127.0.0.1:{port}")]);
    }
    cmd.stdin(Stdio::null()).stderr(Stdio::null());
    cmd.stdout(if has_video { Stdio::piped() } else { Stdio::null() });
    let mut child = cmd.spawn().map_err(|e| format!("Couldn't start ffmpeg: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let appended = Arc::new(AtomicUsize::new(0));
    let audio_present = Arc::new(AtomicBool::new(false));
    let audio_eof = Arc::new(AtomicBool::new(false));
    let video_eof = Arc::new(AtomicBool::new(false));

    let mut frames = None;
    if has_video {
        let mut out = child.stdout.take().unwrap();
        let (tx, rx) = sync_channel::<Frame>(6);
        let (stop, veof) = (stop.clone(), video_eof.clone());
        std::thread::spawn(move || {
            let size = (w * h * 4) as usize;
            let mut n = 0u64;
            while !stop.load(Ordering::SeqCst) {
                let mut data = vec![0u8; size];
                if out.read_exact(&mut data).is_err() {
                    break;
                }
                let pts = at + n as f32 / fps;
                n += 1;
                if tx.send(Frame { pts, data }).is_err() {
                    break;
                }
            }
            veof.store(true, Ordering::SeqCst);
        });
        frames = Some(rx);
    }
    if let (Some(listener), Some(sink)) = (listener, sink.clone()) {
        let (stop, appended, present, eof) = (stop.clone(), appended.clone(), audio_present.clone(), audio_eof.clone());
        std::thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            let deadline = Instant::now() + Duration::from_secs(30);
            let conn = loop {
                match listener.accept() {
                    Ok((c, _)) => break Some(c),
                    Err(_) if Instant::now() < deadline && !stop.load(Ordering::SeqCst) => std::thread::sleep(Duration::from_millis(20)),
                    Err(_) => break None,
                }
            };
            if let Some(mut conn) = conn {
                let _ = conn.set_nonblocking(false);
                let chunk = (44100.0 * 2.0 * 4.0 * CHUNK_SECS) as usize;
                loop {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    while sink.len() > 6 && !stop.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(40));
                    }
                    let mut buf = vec![0u8; chunk];
                    let mut got = 0;
                    while got < chunk {
                        match conn.read(&mut buf[got..]) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => got += n,
                        }
                    }
                    if got == 0 {
                        break;
                    }
                    let samples: Vec<f32> = buf[..got - got % 8].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                    sink.append(SamplesBuffer::new(2, 44100, samples));
                    appended.fetch_add(1, Ordering::SeqCst);
                    present.store(true, Ordering::SeqCst);
                    if got < chunk {
                        break;
                    }
                }
            }
            eof.store(true, Ordering::SeqCst);
        });
    } else {
        audio_eof.store(true, Ordering::SeqCst);
    }

    let mut pipe = Pipeline {
        child,
        frames,
        first: None,
        sink: if has_audio { sink } else { None },
        appended,
        audio_present: audio_present.clone(),
        audio_eof,
        video_eof,
        stop,
        dims: (w, h),
        fps,
    };

    // Wait until the stream is really flowing (first picture and first sound), or fail clearly.
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        if pipe.first.is_none() {
            if let Some(rx) = &pipe.frames {
                if let Ok(f) = rx.recv_timeout(Duration::from_millis(50)) {
                    pipe.first = Some(f);
                }
            }
        }
        let video_ok = pipe.frames.is_none() || pipe.first.is_some();
        let audio_ok = !has_audio || audio_present.load(Ordering::SeqCst) || pipe.audio_eof.load(Ordering::SeqCst);
        if video_ok && audio_ok {
            break;
        }
        if matches!(pipe.child.try_wait(), Ok(Some(_))) && pipe.first.is_none() && !audio_present.load(Ordering::SeqCst) {
            pipe.kill();
            return Err("The stream could not be opened.".into());
        }
        if Instant::now() > deadline {
            pipe.kill();
            return Err("The stream took too long to start.".into());
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    Ok(pipe)
}

/// Same as `out_dims`, but from numbers captured on the main thread (this runs on a worker).
fn out_dims_from(sw: u32, sh: u32, q: Quality, view: (f32, f32)) -> (u32, u32) {
    let (sw, sh) = (sw.max(2) as f32, sh.max(2) as f32);
    let cap_h = q.output_height() as f32;
    let (win_w, win_h) = (view.0.max(1280.0), view.1.max(720.0));
    let (tw, th) = (win_w.min(cap_h * 16.0 / 9.0), win_h.min(cap_h));
    let scale = (tw / sw).min(th / sh).min(1.0);
    (even(sw * scale), even(sh * scale))
}
