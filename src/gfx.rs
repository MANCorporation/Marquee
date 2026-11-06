//! Textures, fonts and drawing helpers. All drawing happens in a 1920x1080 scene space.

use macroquad::prelude::*;
use std::collections::HashMap;

pub const SCENE_W: f32 = 1920.0;
pub const SCENE_H: f32 = 1080.0;
/// Tint applied to the glowing selection bar (the original is plain blue).
pub const ACCENT: (f32, f32, f32) = (0.78, 0.62, 1.0);

const RASTER: u16 = 96;

pub struct IconTex {
    pub sharp: Texture2D,
    pub blurred: Texture2D,
    /// Clear border the blur spreads into, as a share of the picture's side.
    pub blur_margin: f32,
}

pub struct Gfx {
    pub regular: Font,
    pub bold: Font,
    pub icons: HashMap<&'static str, IconTex>,
    pub sel_left: Texture2D,
    pub sel_mid: Texture2D,
    pub sel_right: Texture2D,
    pub yt_logo: Texture2D,
    pub sym: crate::symbols::Symbols,
}

pub struct Decoded {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

/// Decodes an image, shrinking it so its longest side is at most `max_side`.
pub fn decode_image(bytes: &[u8], max_side: u32) -> Option<Decoded> {
    let img = image::load_from_memory(bytes).ok()?;
    Some(shrink(img, max_side))
}

pub fn decode_file(path: &std::path::Path, max_side: u32) -> Option<Decoded> {
    let img = image::open(path).ok()?;
    Some(shrink(img, max_side))
}

fn shrink(img: image::DynamicImage, max_side: u32) -> Decoded {
    let (w, h) = (img.width(), img.height());
    let img = if w.max(h) > max_side {
        img.resize(max_side, max_side, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let rgba = img.to_rgba8();
    Decoded { w: rgba.width(), h: rgba.height(), rgba: rgba.into_raw() }
}

pub fn texture_from(d: &Decoded) -> Texture2D {
    let t = Texture2D::from_rgba8(d.w as u16, d.h as u16, &d.rgba);
    t.set_filter(FilterMode::Linear);
    t
}

pub fn texture_from_bytes(bytes: &[u8]) -> Texture2D {
    match decode_image(bytes, 4096) {
        Some(d) => texture_from(&d),
        None => Texture2D::empty(),
    }
}

/// A small blurred copy of an icon (same look as the original's blurred background icons).
fn blurred_copy(bytes: &[u8]) -> (Texture2D, f32) {
    let img = image::load_from_memory(bytes).expect("icon decodes").to_rgba8();
    let side = 192u32;
    let small = image::imageops::resize(&img, side, side, image::imageops::FilterType::Triangle);
    let sigma = side as f32 * 10.0 / 280.0;
    let margin = (sigma * 3.0).ceil() as u32;
    let mut padded = image::RgbaImage::new(side + margin * 2, side + margin * 2);
    image::imageops::overlay(&mut padded, &small, margin as i64, margin as i64);
    // Blur premultiplied so transparent borders do not bleed dark fringes.
    for p in padded.pixels_mut() {
        let a = p[3] as f32 / 255.0;
        p[0] = (p[0] as f32 * a) as u8;
        p[1] = (p[1] as f32 * a) as u8;
        p[2] = (p[2] as f32 * a) as u8;
    }
    let mut blurred = image::imageops::blur(&padded, sigma);
    for p in blurred.pixels_mut() {
        let a = p[3] as f32 / 255.0;
        if a > 0.001 {
            p[0] = (p[0] as f32 / a).min(255.0) as u8;
            p[1] = (p[1] as f32 / a).min(255.0) as u8;
            p[2] = (p[2] as f32 / a).min(255.0) as u8;
        }
    }
    let t = Texture2D::from_rgba8(blurred.width() as u16, blurred.height() as u16, blurred.as_raw());
    t.set_filter(FilterMode::Linear);
    (t, margin as f32 / side as f32)
}

macro_rules! icon_bytes {
    ($name:literal) => {
        include_bytes!(concat!("../assets/icons/", $name, ".png")) as &[u8]
    };
}

impl Gfx {
    pub fn load() -> Gfx {
        let mut regular = load_ttf_font_from_bytes(include_bytes!("../assets/fonts/Lato-Regular.ttf")).expect("font");
        let mut bold = load_ttf_font_from_bytes(include_bytes!("../assets/fonts/Lato-Bold.ttf")).expect("font");
        regular.set_filter(FilterMode::Linear);
        bold.set_filter(FilterMode::Linear);
        let mut icons = HashMap::new();
        let list: [(&'static str, &[u8]); 5] = [
            ("movies", icon_bytes!("movies")),
            ("music", icon_bytes!("music")),
            ("photos", icon_bytes!("photos")),
            ("settings", icon_bytes!("settings")),
            ("sources", icon_bytes!("sources")),
        ];
        for (name, bytes) in list {
            let (blurred, blur_margin) = blurred_copy(bytes);
            icons.insert(name, IconTex { sharp: texture_from_bytes(bytes), blurred, blur_margin });
        }
        Gfx {
            regular,
            bold,
            icons,
            sel_left: texture_from_bytes(icon_bytes!("bar_left")),
            sel_mid: texture_from_bytes(icon_bytes!("bar_mid")),
            sel_right: texture_from_bytes(icon_bytes!("bar_right")),
            yt_logo: texture_from_bytes(icon_bytes!("youtube_logo")),
            sym: crate::symbols::Symbols::new(),
        }
    }

    pub fn font(&self, bold: bool) -> &Font {
        if bold { &self.bold } else { &self.regular }
    }

    pub fn text_width(&self, text: &str, size: f32, bold: bool) -> f32 {
        measure_text(text, Some(self.font(bold)), RASTER, size / RASTER as f32).width
    }

    /// Draws text with its baseline at `y`.
    pub fn text(&self, text: &str, x: f32, y: f32, size: f32, bold: bool, color: Color) {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(self.font(bold)),
                font_size: RASTER,
                font_scale: size / RASTER as f32,
                color,
                ..Default::default()
            },
        );
    }

    /// Text whose cap-height box is centred vertically on `cy`.
    pub fn text_mid(&self, text: &str, x: f32, cy: f32, size: f32, bold: bool, color: Color) {
        self.text(text, x, cy + size * 0.36, size, bold, color);
    }

    pub fn text_centered(&self, text: &str, cx: f32, cy: f32, size: f32, bold: bool, color: Color) {
        let w = self.text_width(text, size, bold);
        self.text_mid(text, cx - w / 2.0, cy, size, bold, color);
    }

    pub fn text_right(&self, text: &str, right: f32, cy: f32, size: f32, bold: bool, color: Color) {
        let w = self.text_width(text, size, bold);
        self.text_mid(text, right - w, cy, size, bold, color);
    }

    /// Text with a soft drop shadow, truncated with an ellipsis to fit `max_w`.
    pub fn fit(&self, text: &str, max_w: f32, size: f32, bold: bool) -> String {
        if self.text_width(text, size, bold) <= max_w {
            return text.to_string();
        }
        let chars: Vec<char> = text.chars().collect();
        let (mut lo, mut hi) = (0usize, chars.len());
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            let s: String = chars[..mid].iter().collect::<String>() + "…";
            if self.text_width(&s, size, bold) <= max_w {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        chars[..lo].iter().collect::<String>() + "…"
    }
}

pub fn with_alpha(c: Color, a: f32) -> Color {
    Color::new(c.r, c.g, c.b, c.a * a.clamp(0.0, 1.0))
}

pub fn white(a: f32) -> Color {
    Color::new(1.0, 1.0, 1.0, a.clamp(0.0, 1.0))
}

pub fn draw_tex(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color) {
    draw_texture_ex(tex, x, y, color, DrawTextureParams { dest_size: Some(vec2(w, h)), ..Default::default() });
}

pub fn draw_tex_center(tex: &Texture2D, c: Vec2, w: f32, h: f32, alpha: f32) {
    draw_tex(tex, c.x - w / 2.0, c.y - h / 2.0, w, h, white(alpha));
}

/// Draws `tex` fitted (aspect preserved) inside the rect.
pub fn draw_tex_fit(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, alpha: f32) -> Rect {
    let (tw, th) = (tex.width().max(1.0), tex.height().max(1.0));
    let s = (w / tw).min(h / th);
    let (dw, dh) = (tw * s, th * s);
    let (dx, dy) = (x + (w - dw) / 2.0, y + (h - dh) / 2.0);
    draw_tex(tex, dx, dy, dw, dh, white(alpha));
    Rect::new(dx, dy, dw, dh)
}

/// Draws `tex` filling the rect (aspect preserved, cropping the overflow).
pub fn draw_tex_fill(tex: &Texture2D, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
    let (tw, th) = (tex.width().max(1.0), tex.height().max(1.0));
    let s = (w / tw).max(h / th);
    let (sw, sh) = (w / s, h / s);
    let src = Rect::new((tw - sw) / 2.0, (th - sh) / 2.0, sw, sh);
    draw_texture_ex(
        tex,
        x,
        y,
        white(alpha),
        DrawTextureParams { dest_size: Some(vec2(w, h)), source: Some(src), ..Default::default() },
    );
}

/// The picture flipped under itself, fading from `alpha` at the top to nothing 60% of the way down.
pub fn draw_reflection(tex: &Texture2D, c: Vec2, size: f32, alpha: f32) {
    if alpha <= 0.001 || size <= 0.5 {
        return;
    }
    let (x0, x1) = (c.x - size / 2.0, c.x + size / 2.0);
    let y0 = c.y - size / 2.0;
    let y1 = y0 + size * 0.6;
    let top = Color::new(1.0, 1.0, 1.0, alpha);
    let bot = Color::new(1.0, 1.0, 1.0, 0.0);
    quad_mesh(tex, [(x0, y0, 0.0, 1.0, top), (x1, y0, 1.0, 1.0, top), (x1, y1, 1.0, 0.4, bot), (x0, y1, 0.0, 0.4, bot)]);
}

/// Draws a textured quad given its four corners as (x, y, u, v, colour), clockwise from top left.
pub fn quad_mesh(tex: &Texture2D, v: [(f32, f32, f32, f32, Color); 4]) {
    let vertices: Vec<Vertex> = v
        .iter()
        .map(|&(x, y, u, vv, c)| Vertex::new(x, y, 0.0, u, vv, c))
        .collect();
    let mesh = Mesh { vertices, indices: vec![0, 1, 2, 0, 2, 3], texture: Some(tex.clone()) };
    draw_mesh(&mesh);
}

/// A picture seen at an angle: left edge at `x_near` with `h_near`, right edge at `x_far` with `h_far`,
/// both vertically centred on `cy`. Used for the 3D cover walls.
pub fn draw_perspective(tex: &Texture2D, x_near: f32, h_near: f32, x_far: f32, h_far: f32, cy: f32, alpha: f32) {
    let c = white(alpha);
    quad_mesh(
        tex,
        [
            (x_near, cy - h_near / 2.0, 0.0, 0.0, c),
            (x_far, cy - h_far / 2.0, 1.0, 0.0, c),
            (x_far, cy + h_far / 2.0, 1.0, 1.0, c),
            (x_near, cy + h_near / 2.0, 0.0, 1.0, c),
        ],
    );
    // Its reflection on the floor.
    let rc = Color::new(1.0, 1.0, 1.0, alpha * 0.35);
    let z = Color::new(1.0, 1.0, 1.0, 0.0);
    let ry = cy + h_near / 2.0;
    let ry_far = cy + h_far / 2.0;
    quad_mesh(
        tex,
        [
            (x_near, ry, 0.0, 1.0, rc),
            (x_far, ry_far, 1.0, 1.0, rc),
            (x_far, ry_far + h_far * 0.55, 1.0, 0.45, z),
            (x_near, ry + h_near * 0.55, 0.0, 0.45, z),
        ],
    );
}

/// The blue glowing selection bar. `(x, y, w, h)` is the visual frame (row box plus glow margin);
/// the 97px-tall texture is stretched by `h / 75` like the original's `selectionBoxHeight` ratio.
pub fn draw_selection(g: &Gfx, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
    let th = h * 97.0 / 75.0;
    let ty = y + (h - th) / 2.0;
    let cap = 50.0 * th / 97.0;
    let c = Color::new(ACCENT.0, ACCENT.1, ACCENT.2, alpha.clamp(0.0, 1.0));
    draw_tex(&g.sel_left, x, ty, cap, th, c);
    draw_tex(&g.sel_mid, x + cap, ty, (w - cap * 2.0).max(0.0), th, c);
    draw_tex(&g.sel_right, x + w - cap, ty, cap, th, c);
}

/// A chevron with a soft glow, standing in for the SF Symbol "chevron.right".
pub fn draw_chevron(cx: f32, cy: f32, size: f32, color: Color, glow: bool) {
    let h = size * 0.5;
    let w = size * 0.28;
    let pts = [vec2(cx - w, cy - h), vec2(cx + w, cy), vec2(cx - w, cy + h)];
    if glow {
        for (r, a) in [(9.0, 0.10), (6.0, 0.16), (3.5, 0.3)] {
            let gc = Color::new(0.65, 0.5, 1.0, a * color.a);
            draw_line(pts[0].x, pts[0].y, pts[1].x, pts[1].y, r * 2.0, gc);
            draw_line(pts[1].x, pts[1].y, pts[2].x, pts[2].y, r * 2.0, gc);
        }
    }
    let t = size * 0.17;
    draw_line(pts[0].x, pts[0].y, pts[1].x, pts[1].y, t, color);
    draw_line(pts[1].x, pts[1].y, pts[2].x, pts[2].y, t, color);
    draw_circle(pts[0].x, pts[0].y, t / 2.0, color);
    draw_circle(pts[2].x, pts[2].y, t / 2.0, color);
    draw_circle(pts[1].x, pts[1].y, t / 2.0, color);
}

pub fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Deep indigo-to-black backdrop with a faint glow behind the carousel.
pub fn draw_background() {
    let top = Color::new(0.07, 0.05, 0.16, 1.0);
    let bot = Color::new(0.0, 0.0, 0.02, 1.0);
    let v = |x: f32, y: f32, c: Color| Vertex::new(x, y, 0.0, 0.0, 0.0, c);
    let mesh = Mesh {
        vertices: vec![v(0.0, 0.0, top), v(SCENE_W, 0.0, top), v(SCENE_W, SCENE_H, bot), v(0.0, SCENE_H, bot)],
        indices: vec![0, 1, 2, 0, 2, 3],
        texture: None,
    };
    draw_mesh(&mesh);
    // Darker edges (stacked thin strips; plain alpha rectangles blend reliably).
    for i in 0..26 {
        let a = 0.5 * (1.0 - i as f32 / 26.0).powi(2);
        let c = Color::new(0.0, 0.0, 0.0, a);
        draw_rectangle(i as f32 * 8.0, 0.0, 8.0, SCENE_H, c);
        draw_rectangle(SCENE_W - (i as f32 + 1.0) * 8.0, 0.0, 8.0, SCENE_H, c);
    }
    // A soft glow: many faint, nearly invisible steps so no banding shows.
    for i in 0..48 {
        let r = 620.0 - i as f32 * 12.0;
        draw_circle(430.0, 440.0, r, Color::new(0.45, 0.3, 0.9, 0.0045));
    }
}
