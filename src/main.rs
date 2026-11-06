mod anim;
mod apps;
mod app;
mod audio;
mod carousel;
mod draw;
mod entries;
mod gfx;
mod library;
mod menu;
mod net;
mod netui;
mod ffmpeg;
mod screens;
mod icon_data;
mod settings;
mod symbols;
mod synth;
mod texcache;
mod video;
mod videoplayer;

use app::{App, Key};
use macroquad::prelude::*;

fn window_icon() -> Option<macroquad::miniquad::conf::Icon> {
    fn rgba(png: &[u8], px: u32) -> Option<Vec<u8>> {
        let img = image::load_from_memory(png).ok()?.resize_exact(px, px, image::imageops::FilterType::Lanczos3);
        Some(img.to_rgba8().into_raw())
    }
    let big = include_bytes!("../assets/appicon/marquee-256.png");
    Some(macroquad::miniquad::conf::Icon {
        small: rgba(big, 16)?.try_into().ok()?,
        medium: rgba(big, 32)?.try_into().ok()?,
        big: rgba(big, 64)?.try_into().ok()?,
    })
}

fn conf() -> Conf {
    Conf {
        icon: window_icon(),
        window_title: "Marquee".to_string(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    }
}

/// Keyboard state with hold-to-repeat for the arrow keys (first repeat after 0.5 s, then accelerating).
struct Input {
    held: Option<(KeyCode, Key, f64, f64)>, // (code, key, pressed_at, next_repeat)
}

impl Input {
    fn poll(&mut self, now: f64, long_list: bool, sweeping: &mut bool) -> Vec<Key> {
        let mut out = vec![];
        let modifier = is_key_down(KeyCode::LeftSuper) || is_key_down(KeyCode::RightSuper) || is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if modifier && (is_key_pressed(KeyCode::Escape) || is_key_pressed(KeyCode::Q)) {
            out.push(Key::Quit);
            return out;
        }
        let dirs = [(KeyCode::Up, Key::Up), (KeyCode::Down, Key::Down), (KeyCode::Left, Key::Left), (KeyCode::Right, Key::Right)];
        for (code, key) in dirs {
            if is_key_pressed(code) {
                out.push(key);
                self.held = Some((code, key, now, now + 0.5));
            }
        }
        if let Some((code, key, t0, next)) = self.held {
            if !is_key_down(code) {
                self.held = None;
                *sweeping = false;
            } else if now >= next {
                let held_for = now - t0;
                let interval = if held_for > 1.0 { if long_list { 0.04 } else { 0.08 } } else { 0.16 };
                out.push(key);
                *sweeping = true;
                self.held = Some((code, key, t0, now + interval));
            }
        } else {
            *sweeping = false;
        }
        for (code, key) in [
            (KeyCode::Enter, Key::Enter),
            (KeyCode::KpEnter, Key::Enter),
            (KeyCode::Space, Key::Space),
            (KeyCode::C, Key::Channel),
            (KeyCode::Escape, Key::Back),
            (KeyCode::Backspace, Key::Backspace),
            (KeyCode::Delete, Key::Backspace),
        ] {
            if is_key_pressed(code) {
                out.push(key);
            }
        }
        out
    }
}

fn parse_key(s: &str) -> Option<Key> {
    Some(match s {
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "enter" => Key::Enter,
        "back" => Key::Back,
        "space" => Key::Space,
        "channel" => Key::Channel,
        _ => return None,
    })
}

/// `--script "wait:1.5,down,down,enter" --shot out.png --shot-at 6` drives the app for screenshots/tests.
struct Script {
    steps: Vec<(f64, Key)>,
    chars: Vec<(f64, char)>,
    shots: Vec<(f64, String)>,
    exit_at: Option<f64>,
}

fn parse_args() -> Script {
    let args: Vec<String> = std::env::args().collect();
    let mut script = Script { steps: vec![], chars: vec![], shots: vec![], exit_at: None };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--script" if i + 1 < args.len() => {
                let mut t = 1.0;
                for tok in args[i + 1].split(',') {
                    if let Some(w) = tok.strip_prefix("wait:") {
                        t += w.parse::<f64>().unwrap_or(0.0);
                    } else if let Some(text) = tok.strip_prefix("type:") {
                        for c in text.chars() {
                            script.chars.push((t, if c == '_' { ' ' } else { c }));
                            t += 0.03;
                        }
                        t += 0.1;
                    } else if let Some(k) = parse_key(tok) {
                        script.steps.push((t, k));
                        t += 0.15;
                    }
                }
                i += 1;
            }
            "--shot" if i + 1 < args.len() => {
                // path@seconds
                let spec = &args[i + 1];
                let (p, at) = spec.rsplit_once('@').unwrap_or((spec, "5"));
                script.shots.push((at.parse().unwrap_or(5.0), p.to_string()));
                i += 1;
            }
            "--exit-at" if i + 1 < args.len() => {
                script.exit_at = args[i + 1].parse().ok();
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }
    script
}

fn scene_camera() -> Camera2D {
    let (sw, sh) = (screen_width(), screen_height());
    let scale = (sw / gfx::SCENE_W).min(sh / gfx::SCENE_H);
    let (fw, fh) = (gfx::SCENE_W * scale, gfx::SCENE_H * scale);
    let dpi = screen_dpi_scale();
    let mut cam = Camera2D::from_display_rect(Rect::new(0.0, 0.0, gfx::SCENE_W, gfx::SCENE_H));
    // from_display_rect has a negative y zoom, which draws the scene upside down; flip it back.
    cam.zoom.y = -cam.zoom.y;
    cam.viewport = Some((
        (((sw - fw) / 2.0) * dpi) as i32,
        (((sh - fh) / 2.0) * dpi) as i32,
        (fw * dpi) as i32,
        (fh * dpi) as i32,
    ));
    cam
}

#[macroquad::main(conf)]
async fn main() {
    // Diagnostics: `marquee --test-stream <url> [seconds]` resolves a web video and times a streamed decode.
    let argv: Vec<String> = std::env::args().collect();
    if argv.get(1).map(|s| s.as_str()) == Some("--test-stream") && argv.len() >= 3 {
        let st = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let secs: u32 = argv.get(3).and_then(|s| s.parse().ok()).unwrap_or(10);
        let t0 = std::time::Instant::now();
        match apps::resolve_stream(&argv[2], 2160, &st) {
            Ok(s) => {
                println!("resolved in {:.1}s: {}x{} @{} fps, {:.0}s, audio={}", t0.elapsed().as_secs_f32(), s.width, s.height, s.fps, s.duration, s.audio.is_some());
                if let (Some(v), Some(ffm)) = (s.video.as_ref(), ffmpeg::tool("ffmpeg")) {
                    let mut c = std::process::Command::new(ffm);
                    c.args(["-v", "error", "-nostdin", "-hwaccel", "auto"]);
                    for (k, val) in &v.headers {
                        if k.eq_ignore_ascii_case("user-agent") {
                            c.args(["-user_agent", val]);
                        }
                    }
                    let t1 = std::time::Instant::now();
                    let ok = c.args(["-ss", "600", "-i", &v.url, "-t", &secs.to_string(), "-an", "-f", "null", "-"]).status().map(|s| s.success()).unwrap_or(false);
                    println!("decoded {secs}s of media (seeking to 10:00) in {:.1}s ok={ok}", t1.elapsed().as_secs_f32());
                }
            }
            Err(e) => println!("FAIL: {e}"),
        }
        return;
    }
    // Dev tool: `marquee --render-svg in.svg out.png size`
    if argv.get(1).map(|s| s.as_str()) == Some("--render-svg") && argv.len() >= 5 {
        let svg = std::fs::read_to_string(&argv[2]).expect("read svg");
        let px: u32 = argv[4].parse().unwrap_or(256);
        let (w, h, data) = symbols::render_svg_wh(&svg, px).expect("render");
        image::save_buffer(&argv[3], &data, w, h, image::ColorType::Rgba8).expect("write png");
        return;
    }
    let script = parse_args();
    let mut app = App::new();
    let mut input = Input { held: None };
    let start = get_time();
    let mut step = 0usize;
    let mut cstep = 0usize;
    let mut shots_done = vec![false; script.shots.len()];
    let mut fullscreen = false;
    app.last_input = 0.0;
    loop {
        let now = get_time() - start;
        let mut keys = {
            let mut sweeping = app.linear_sweep;
            let long = false;
            let k = input.poll(now, long, &mut sweeping);
            app.linear_sweep = sweeping;
            k
        };
        while step < script.steps.len() && script.steps[step].0 <= now {
            keys.push(script.steps[step].1);
            step += 1;
        }
        if is_key_pressed(KeyCode::F11) || (is_key_down(KeyCode::LeftSuper) && is_key_pressed(KeyCode::F)) || (is_key_down(KeyCode::LeftAlt) && is_key_pressed(KeyCode::Enter)) {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }
        let mut chars = vec![];
        while let Some(c) = get_char_pressed() {
            if !c.is_control() {
                chars.push(c);
            }
        }
        while cstep < script.chars.len() && script.chars[cstep].0 <= now {
            chars.push(script.chars[cstep].1);
            cstep += 1;
        }
        app.update(now, &keys, &chars);
        if app.quit {
            break;
        }

        clear_background(BLACK);
        {
            let (sw, sh) = (screen_width(), screen_height());
            let s = (sw / gfx::SCENE_W).min(sh / gfx::SCENE_H);
            let dpi = screen_dpi_scale();
            symbols::set_view(s * dpi, (((sw - gfx::SCENE_W * s) / 2.0) * dpi).floor(), (((sh - gfx::SCENE_H * s) / 2.0) * dpi).floor());
        }
        set_camera(&scene_camera());
        app.draw_screen(now);
        // Black overlay used for transitions.
        let fade = app.fade.value(now);
        if fade > 0.001 {
            draw_rectangle(0.0, 0.0, gfx::SCENE_W, gfx::SCENE_H, Color::new(0.0, 0.0, 0.0, fade));
        }
        if let Some((msg, _)) = &app.toast {
            app.g.text_centered(msg, gfx::SCENE_W / 2.0, 990.0, 32.0, false, gfx::white(0.8));
        }
        set_default_camera();

        for (i, (at, path)) in script.shots.iter().enumerate() {
            if !shots_done[i] && now >= *at {
                let img = get_screen_data();
                img.export_png(path);
                shots_done[i] = true;
            }
        }
        if let Some(t) = script.exit_at {
            if now >= t {
                break;
            }
        }
        next_frame().await;
    }
}
