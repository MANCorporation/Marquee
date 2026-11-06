//! Adwaita symbolic icons, rasterised from SVG at the exact size they are shown, so every glyph is crisp and
//! lands on whole device pixels (the icons are drawn on a 16-unit grid and look wrong when resampled).

use crate::icon_data::ICONS;
use macroquad::prelude::*;
use resvg::{tiny_skia, usvg};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

static SCALE: AtomicU32 = AtomicU32::new(0x3f800000); // 1.0f32 bits
static OFF_X: AtomicU32 = AtomicU32::new(0);
static OFF_Y: AtomicU32 = AtomicU32::new(0);

/// Scene-to-device-pixel mapping for the frame being drawn (set by the main loop).
pub fn set_view(scale: f32, off_x: f32, off_y: f32) {
    SCALE.store(scale.to_bits(), Ordering::Relaxed);
    OFF_X.store(off_x.to_bits(), Ordering::Relaxed);
    OFF_Y.store(off_y.to_bits(), Ordering::Relaxed);
}

fn view() -> (f32, f32, f32) {
    (
        f32::from_bits(SCALE.load(Ordering::Relaxed)).max(0.01),
        f32::from_bits(OFF_X.load(Ordering::Relaxed)),
        f32::from_bits(OFF_Y.load(Ordering::Relaxed)),
    )
}

/// Renders an SVG at `px` wide (height follows its aspect ratio); returns (width, height, straight-alpha RGBA).
pub fn render_svg_wh(svg: &str, px: u32) -> Option<(u32, u32, Vec<u8>)> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let s = tree.size();
    let h = ((px as f32) * s.height() / s.width()).round().max(1.0) as u32;
    rasterize_wh(&tree, px, h).map(|d| (px, h, d))
}

/// Renders an SVG string to straight-alpha RGBA at `px` x `px` (the viewBox is scaled to fit).
pub fn render_svg(svg: &str, px: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    rasterize(&tree, px)
}

fn rasterize(tree: &usvg::Tree, px: u32) -> Option<Vec<u8>> {
    rasterize_wh(tree, px, px)
}

fn rasterize_wh(tree: &usvg::Tree, pw: u32, ph: u32) -> Option<Vec<u8>> {
    let (px, py) = (pw, ph);
    let mut pixmap = tiny_skia::Pixmap::new(px, py)?;
    let s = tree.size();
    let k = (px as f32 / s.width()).min(py as f32 / s.height());
    let (dx, dy) = ((px as f32 - s.width() * k) / 2.0, (py as f32 - s.height() * k) / 2.0);
    let tf = tiny_skia::Transform::from_scale(k, k).post_translate(dx, dy);
    resvg::render(tree, tf, &mut pixmap.as_mut());
    // tiny-skia is premultiplied; textures want straight alpha.
    let mut out = pixmap.take();
    for p in out.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Some(out)
}

pub struct Symbols {
    trees: HashMap<&'static str, usvg::Tree>,
    cache: RefCell<HashMap<(&'static str, u32), Texture2D>>,
}

impl Symbols {
    pub fn new() -> Symbols {
        let mut trees = HashMap::new();
        for (name, svg) in ICONS {
            // Symbolic icons are drawn in #2e3436; make them white so they can be tinted at draw time.
            let white = svg.replace("#2e3436", "#ffffff");
            if let Ok(t) = usvg::Tree::from_str(&white, &usvg::Options::default()) {
                trees.insert(*name, t);
            }
        }
        Symbols { trees, cache: RefCell::new(HashMap::new()) }
    }

    fn texture(&self, name: &str, px: u32) -> Option<Texture2D> {
        let (key, tree) = self.trees.get_key_value(name)?;
        if let Some(t) = self.cache.borrow().get(&(*key, px)) {
            return Some(t.clone());
        }
        let data = rasterize(tree, px)?;
        let t = Texture2D::from_rgba8(px as u16, px as u16, &data);
        t.set_filter(FilterMode::Linear);
        let mut c = self.cache.borrow_mut();
        if c.len() > 300 {
            c.clear();
        }
        c.insert((*key, px), t.clone());
        Some(t)
    }

    /// Draws the icon centred on (cx, cy), `size` scene units square, in `color`.
    pub fn draw(&self, name: &str, cx: f32, cy: f32, size: f32, color: Color) {
        if color.a <= 0.003 {
            return;
        }
        let (scale, ox, oy) = view();
        // Whole device pixels for both the size (even, so the glyph stays centred) and the position.
        let mut px = (size * scale).round().max(4.0);
        if (px as u32) % 2 == 1 {
            px += 1.0;
        }
        let Some(tex) = self.texture(name, px as u32) else { return };
        let dev_x = (ox + cx * scale - px / 2.0).round();
        let dev_y = (oy + cy * scale - px / 2.0).round();
        let (x, y, d) = ((dev_x - ox) / scale, (dev_y - oy) / scale, px / scale);
        draw_texture_ex(&tex, x, y, color, DrawTextureParams { dest_size: Some(vec2(d, d)), ..Default::default() });
    }
}
