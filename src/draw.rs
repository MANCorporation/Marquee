//! Drawing the menu scene: carousel, header, and the three kinds of list.

use crate::anim::{ease_in_out, Tween};
use crate::app::*;
use crate::carousel::{self, CarouselConfig, IconState};
use crate::gfx::*;
use crate::texcache::{Source, TexCache};
use macroquad::prelude::*;

pub struct Frames {
    pub carousel: Rect,
    pub menu: Vec2,
    pub menu_w: f32,
    pub header_top: f32,
}

pub fn frames() -> Frames {
    let carousel_lane = (ICON_SIZE * 7.0 + 80.0 * 6.0).min(SCENE_W * 0.67);
    let menu_lane = MENU_WIDTH.min(SCENE_W * 0.35 + 100.0);
    let carousel_h = ICON_SIZE * SELECTED_MULT + 100.0;
    let pitch = SELECTION_BOX_HEIGHT + ROW_SPACING;
    let stable_h = DEFAULT_VISIBLE_ROWS as f32 * pitch + LIST_BOTTOM_INSET;
    let row_h = carousel_h.max(stable_h);
    let title_h = 72.0;
    let column_h = title_h + 35.3046875 + row_h;
    let title_top = (SCENE_H - column_h) * 0.5;
    let row_top = title_top + title_h + 35.3046875;
    let content_w = carousel_lane + 8.0 + menu_lane;
    let (carousel_x, menu_x) = if content_w > SCENE_W {
        let start = (SCENE_W - content_w) * 0.5;
        (start, start + carousel_lane + 8.0)
    } else {
        (0.0, SCENE_W - menu_lane)
    };
    let comp = (DEFAULT_VISIBLE_ROWS - 7) as f32 * pitch * 0.5;
    let carousel_y = row_top + (row_h - carousel_h) * 0.5 - ARC_RADIUS * 0.3 * 1.5;
    Frames {
        carousel: Rect::new(carousel_x + 40.0, carousel_y, carousel_lane, carousel_h),
        menu: vec2(menu_x - 214.0, row_top + comp),
        menu_w: menu_lane,
        header_top: title_top - 10.0 + comp,
    }
}

fn carousel_config(sel: usize, f: &Frames, landed: Vec2, animated: bool, entering: bool) -> CarouselConfig {
    CarouselConfig {
        selected_index: sel as i32,
        is_icon_animated: animated,
        is_entering_submenu: entering,
        is_returning_to_root: false,
        is_in_submenu: false,
        icon_size: ICON_SIZE,
        scale_reduction: SCALE_REDUCTION,
        landed_icon_scale: LANDED_SCALE,
        selected_size_multiplier: SELECTED_MULT,
        arc_radius: ARC_RADIUS,
        resting_offset: vec2(ICON_SIZE * 0.75, -(0.1 * ICON_SIZE)),
        landed_offset: landed,
        entry_offset: ENTRY_OFFSET,
        scene_width: SCENE_W,
        box_origin: vec2(f.carousel.x, f.carousel.y),
    }
}

pub fn draw_icon(g: &Gfx, name: &str, origin: Vec2, st: &IconState, extra_alpha: f32) {
    let Some(icon) = g.icons.get(name) else { return };
    let blur = (st.blur_radius / 10.0).clamp(0.0, 1.0);
    let center = origin + st.center;
    let m = icon.blur_margin;
    if st.shows_reflection {
        let rc = center + st.reflection_center;
        let a = st.opacity * st.reflection_opacity * extra_alpha;
        draw_reflection(&icon.sharp, rc, st.reflection_size, a * (1.0 - blur));
        if blur > 0.0 {
            draw_reflection(&icon.blurred, rc, st.reflection_size * (1.0 + 2.0 * m), a * blur);
        }
    }
    let a = st.opacity * extra_alpha;
    if blur < 1.0 {
        draw_tex_center(&icon.sharp, center, st.size, st.size, a * (1.0 - blur));
    }
    if blur > 0.0 {
        let s = st.size * (1.0 + 2.0 * m);
        draw_tex_center(&icon.blurred, center, s, s, a * blur);
    }
}

fn draw_shuffle_glyph(cx: f32, cy: f32, color: Color) {
    let w = 15.0;
    let t = 3.0;
    draw_line(cx - w, cy - 8.0, cx - 3.0, cy - 8.0, t, color);
    draw_line(cx - 3.0, cy - 8.0, cx + 3.0, cy + 8.0, t, color);
    draw_line(cx + 3.0, cy + 8.0, cx + w, cy + 8.0, t, color);
    draw_line(cx - w, cy + 8.0, cx - 3.0, cy + 8.0, t, color);
    draw_line(cx - 3.0, cy + 8.0, cx + 3.0, cy - 8.0, t, color);
    draw_line(cx + 3.0, cy - 8.0, cx + w, cy - 8.0, t, color);
    draw_triangle(vec2(cx + w + 6.0, cy - 8.0), vec2(cx + w - 3.0, cy - 14.0), vec2(cx + w - 3.0, cy - 2.0), color);
    draw_triangle(vec2(cx + w + 6.0, cy + 8.0), vec2(cx + w - 3.0, cy + 2.0), vec2(cx + w - 3.0, cy + 14.0), color);
}

/// One scrolling list with the glowing selection box.
pub fn draw_list(g: &Gfx, tex: &mut TexCache, view: &mut ListView, rows: &[Row], sel: usize, alpha: f32, now: f64, origin: Vec2, width: f32, visible: usize) {
    if alpha <= 0.002 || rows.is_empty() {
        return;
    }
    let sel = sel.min(rows.len() - 1);
    let pitch = SELECTION_BOX_HEIGHT + ROW_SPACING;
    let mut offsets = Vec::with_capacity(rows.len());
    let mut y = 0.0;
    for r in rows {
        if r.divider {
            y += DIVIDER_GAP;
        }
        offsets.push(y);
        y += pitch;
    }
    let content_h = offsets.last().copied().unwrap_or(0.0) + SELECTION_BOX_HEIGHT;
    let viewport = visible as f32 * pitch;
    let scroll_target = if content_h > viewport {
        let anchor = offsets[STICKY_ROW.min(rows.len() - 1)];
        (anchor - offsets[sel]).clamp(viewport - content_h, 0.0)
    } else {
        0.0
    };
    let sel_target = offsets[sel];
    if view.last_target != Some((sel_target, scroll_target)) {
        match (&mut view.sel_y, &mut view.scroll) {
            (Some(s), Some(c)) => {
                s.go(now, sel_target, SELECTION_ANIM);
                c.go(now, scroll_target, SELECTION_ANIM);
            }
            _ => {
                view.sel_y = Some(Tween::new(sel_target));
                view.scroll = Some(Tween::new(scroll_target));
            }
        }
        view.last_target = Some((sel_target, scroll_target));
    }
    let sel_off = view.sel_y.as_ref().map(|t| t.value(now)).unwrap_or(sel_target);
    let scroll = view.scroll.as_ref().map(|t| t.value(now)).unwrap_or(scroll_target);

    // Selection box (visual frame is the row box plus a glow margin).
    draw_selection(g, origin.x - 46.5, origin.y + sel_off + scroll - 29.0, width + 93.0, SELECTION_BOX_HEIGHT + 58.0, alpha);

    // Leading column: thumbnails if any row has one, otherwise an icon column if most rows have an icon.
    let any_thumb = rows.iter().any(|r| r.thumb.is_some());
    let icon_col = !any_thumb && rows.iter().filter(|r| r.icon.is_some()).count() * 2 >= rows.len();
    if rows.len() == 1 && rows[0].title == "Loading…" {
        // A spinner while a list is being fetched.
        let c = vec2(origin.x + 56.0, origin.y + SELECTION_BOX_HEIGHT / 2.0);
        for k in 0..10 {
            let a = (now as f32 * 7.0 - k as f32 * 0.55).rem_euclid(std::f32::consts::TAU);
            let ang = a;
            let fade = 1.0 - k as f32 / 10.0;
            draw_circle(c.x + ang.cos() * 16.0, c.y + ang.sin() * 16.0, 3.5, white(fade * alpha));
        }
    }
    for (i, row) in rows.iter().enumerate() {
        let ry = origin.y + offsets[i] + scroll;
        // Rows near the viewport's edges fade out instead of being clipped.
        let top_vis = ((ry + SELECTION_BOX_HEIGHT - origin.y) / SELECTION_BOX_HEIGHT).clamp(0.0, 1.0);
        let bot_vis = ((origin.y + viewport - ry) / SELECTION_BOX_HEIGHT).clamp(0.0, 1.0);
        let vis = top_vis.min(bot_vis) * alpha;
        if vis <= 0.002 {
            continue;
        }
        let cy = ry + SELECTION_BOX_HEIGHT / 2.0;
        let active = i == sel;
        if row.divider {
            let ly = ry - DIVIDER_GAP + 2.0;
            draw_line(origin.x + 33.0, ly, origin.x + width - 33.0, ly, 1.5, white(0.34 * vis));
        }
        let right = origin.x + width - 35.0;
        let mut text_right_limit = right;
        if row.leads {
            let ax = origin.x + width - 37.0;
            if active {
                // A soft accent halo behind the glyph.
                g.sym.draw("go-next", ax, cy, 50.0, Color::new(0.62, 0.48, 1.0, 0.28 * vis));
                g.sym.draw("go-next", ax, cy, 40.0, white(vis));
            } else {
                g.sym.draw("go-next", ax, cy, 40.0, white(0.5 * vis));
            }
            text_right_limit = ax - 24.0;
        }
        let mut trailing_w = 0.0;
        if let Some(t) = &row.trailing {
            let is_dots = t == "•••";
            let size = if is_dots { 17.0 } else { 32.0 };
            let t = g.fit(t, width * 0.34, size, false);
            let tw = g.text_width(&t, size, false);
            trailing_w = tw + 20.0;
            let a = if active { vis } else { 0.5 * vis };
            g.text_right(&t, text_right_limit, cy, size, false, white(a));
        }
        let mut title_x = origin.x + 35.0;
        if icon_col {
            title_x = origin.x + 92.0;
        }
        if let Some(name) = row.icon.filter(|_| row.thumb.is_none() && (icon_col || any_thumb)) {
            let icon_cx = if any_thumb { origin.x + 16.0 + 50.0 } else { origin.x + 50.0 };
            g.sym.draw(name, icon_cx, cy, 44.0, white(vis * if active { 1.0 } else { 0.78 }));
            if any_thumb {
                title_x = origin.x + 16.0 + 100.0 + 18.0;
            }
        }
        if any_thumb && row.thumb.is_none() && row.icon.is_none() {
            title_x = origin.x + 16.0 + 100.0 + 18.0;
        }
        if let Some((key, src)) = &row.thumb {
            let (tw, th) = (100.0, 56.0);
            let tx = origin.x + 16.0;
            let ty = cy - th / 2.0;
            draw_rectangle(tx, ty, tw, th, white(0.08 * vis));
            if let Some(Some(t)) = tex.get(key, src.clone(), 360) {
                draw_tex_fill(t, tx, ty, tw, th, vis);
            }
            title_x = tx + tw + 18.0;
        }
        let avail = (width - (title_x - origin.x) - 35.0 - trailing_w - if row.leads { 40.0 } else { 0.0 }).max(80.0);
        let title = g.fit(&row.title, avail, 40.0, true);
        g.text_mid(&title, title_x + if rows.len() == 1 && row.title == "Loading…" { 50.0 } else { 0.0 }, cy, 40.0, true, white(vis * if active { 1.0 } else { 0.86 }));
    }
}

impl App {
    fn header_text(&self) -> String {
        let h = self.header.split_once(" › ").map(|(_, b)| b).unwrap_or(&self.header);
        self.g.fit(h, 520.0, 60.0, true)
    }

    /// The target position of the landed (small) section icon beside the header title.
    fn landed_center(&self, f: &Frames) -> Vec2 {
        let title_w = self.g.text_width(&self.header_text(), 60.0, true);
        let mid_x = f.menu.x + f.menu_w / 2.0;
        let center_x = mid_x + ICON_SIZE * SELECTED_MULT * LANDED_SCALE * 0.6;
        vec2(center_x - title_w / 2.0 - 71.0, f.header_top + 29.0)
    }

    pub fn draw_menu(&mut self, now: f64) {
        draw_background();
        let f = frames();
        let bounds = vec2(f.carousel.w, f.carousel.h);
        let landed_abs = self.landed_center(&f);
        let landed_off = vec2(
            landed_abs.x - (f.carousel.x + f.carousel.w / 2.0),
            landed_abs.y - (f.carousel.y + f.carousel.h / 2.0 - 50.0),
        );
        let r = carousel_config(self.sel, &f, landed_off, false, false);
        let e_cfg = carousel_config(self.sel, &f, landed_off, true, true);
        let e = self.enter.value(now);
        let phase = self.phase.value(now);
        let origin = vec2(f.carousel.x, f.carousel.y);
        let settled = e >= 0.999 && self.level != Level::Root;
        let yt = self.level != Level::Root && self.header.starts_with("YouTube");
        let order = carousel::draw_order(self.roots.len(), self.sel as i32, e > 0.001);
        for idx in order {
            let sr = carousel::state_at_phase(idx, phase, &r, bounds);
            let se = carousel::state_at_phase(idx, self.sel as f32, &e_cfg, bounds);
            let mut st = IconState::lerp(&sr, &se, e);
            if idx != self.sel {
                if settled {
                    continue;
                }
                st.opacity *= 1.0 - ease_in_out((e * 2.0).min(1.0));
            } else if settled {
                st.shows_reflection = false;
                if yt {
                    continue;
                }
            }
            draw_icon(&self.g, self.roots[idx].icon, origin, &st, 1.0);
        }

        // Left-hand preview content while in a section.
        let sub_a = self.sub_alpha.value(now);
        if sub_a > 0.002 && self.level != Level::Root || (sub_a > 0.002 && e > 0.5) {
            self.draw_gap_content(now, &f, &r, bounds, origin, sub_a, yt);
        }

        // Header.
        let root_a = self.root_alpha.value(now);
        let mid_x = f.menu.x + f.menu_w / 2.0;
        let header_cy = f.header_top + 36.0;
        if root_a > 0.002 {
            self.g.text_centered("Marquee", mid_x, header_cy, 60.0, true, white(root_a));
        }
        if sub_a > 0.002 && yt {
            draw_youtube_logo(&self.g, mid_x, header_cy - 4.0, sub_a);
            if let Some((_, sub)) = self.header.split_once(" › ") {
                let sub = self.g.fit(sub, 640.0, 30.0, false);
                self.g.text_centered(&sub, mid_x, header_cy + 58.0, 30.0, false, white(0.6 * sub_a));
            }
        } else if sub_a > 0.002 {
            let cx = mid_x + ICON_SIZE * SELECTED_MULT * LANDED_SCALE * 0.6;
            self.g.text_centered(&self.header_text(), cx, header_cy, 60.0, true, white(sub_a));
        }

        if yt && self.level == Level::Third {
            if let Some(Entry { action: Action::PlayPage(it), .. }) = self.thirds.last().and_then(|t| t.entries.get(t.sel)) {
                if it.channel_url.is_some() {
                    self.g.text_centered("Press C to open this video's channel", SCENE_W / 2.0, 1010.0, 26.0, false, white(0.4 * sub_a));
                }
            }
        }

        // Lists.
        let root_rows = self.root_rows();
        let mut v0 = std::mem::take(&mut self.views[0]);
        draw_list(&self.g, &mut self.tex, &mut v0, &root_rows, self.sel, root_a, now, f.menu, f.menu_w, DEFAULT_VISIBLE_ROWS);
        self.views[0] = v0;
        if self.level == Level::Third {
            let rows = self.third_rows();
            let sel = self.thirds.last().map(|t| t.sel).unwrap_or(0);
            let mut v2 = std::mem::take(&mut self.views[2]);
            draw_list(&self.g, &mut self.tex, &mut v2, &rows, sel, sub_a, now, f.menu, f.menu_w, THIRD_VISIBLE_ROWS);
            self.views[2] = v2;
        } else if self.level == Level::Sub || sub_a > 0.002 {
            let rows = self.sub_rows();
            let mut v1 = std::mem::take(&mut self.views[1]);
            draw_list(&self.g, &mut self.tex, &mut v1, &rows, self.sub_sel, sub_a, now, f.menu, f.menu_w, DEFAULT_VISIBLE_ROWS);
            self.views[1] = v1;
            if rows.is_empty() && self.level == Level::Sub && sub_a > 0.5 {
                let msg = if self.roots[self.active_root].id == "podcasts" { "Podcasts require Apple Podcasts,\nwhich isn't available here." } else { "Nothing here" };
                for (i, line) in msg.lines().enumerate() {
                    self.g.text_centered(line, mid_x, f.menu.y + 120.0 + i as f32 * 48.0, 34.0, false, white(0.6 * sub_a));
                }
            }
        }
    }

    fn draw_gap_content(&mut self, now: f64, f: &Frames, r: &CarouselConfig, bounds: Vec2, origin: Vec2, alpha: f32, yt: bool) {
        let _ = f;
        let id = self.roots[self.active_root].icon;
        // Photo albums and song lists show a perspective wall of pictures; everything else the reflected icon.
        if self.level == Level::Third && self.draw_wall(now, alpha) {
            return;
        }
        if yt {
            return;
        }
        let st = carousel::slot_state(0, r, bounds);
        draw_icon(&self.g, id, origin, &st, alpha);
    }

    /// What the left-hand wall should show: an id (the wall fades when it changes), the continuous position
    /// to scroll to, and the pictures around it as (absolute index, texture key, source).
    fn wall_plan(&self) -> Option<(String, f32, Vec<(f32, String, Source)>)> {
        let third = self.thirds.last()?;
        let mut units: Vec<(f32, String, Source)> = vec![];
        // The YouTube menu previews the highlighted list as a static fan of thumbnails.
        if third.title == "YouTube" {
            let entry = third.entries.get(third.sel)?;
            let thumbs: Vec<String> = match &entry.action {
                Action::YtCategory(c) => match self.yt_cache.get(c) {
                    Some(YtSlot::Ready(items)) => items.iter().filter_map(|i| i.thumb.clone()).take(4).collect(),
                    _ => vec![],
                },
                Action::YtHistory => self.settings.history.iter().filter_map(|i| i.thumb.clone()).take(4).collect(),
                Action::YtSubs => self.settings.subs.iter().filter_map(|c| c.thumb.clone()).take(4).collect(),
                _ => vec![],
            };
            for (i, t) in thumbs.into_iter().enumerate() {
                units.push((i as f32, format!("u:{t}"), Source::Url(t)));
            }
            return if units.is_empty() { None } else { Some((format!("yt:{}", third.sel), 0.0, units)) };
        }
        if !third.tracks.is_empty() {
            // One picture per album run, so a track list scrolls through covers rather than repeating one.
            let t = &third.tracks;
            let sel = third.sel.min(t.len() - 1);
            let mut unit = 0usize;
            for i in 1..=sel {
                if t[i].album != t[i - 1].album || t[i].path.parent() != t[i - 1].path.parent() {
                    unit += 1;
                }
            }
            units.push((unit as f32, format!("c:{}", t[sel].path.display()), Source::Cover(t[sel].path.clone())));
            // Walk back and forward collecting neighbouring covers.
            let (mut u, mut i) = (unit, sel);
            while i > 0 && units.len() < 3 {
                i -= 1;
                if t[i].album != t[i + 1].album || t[i].path.parent() != t[i + 1].path.parent() {
                    u -= 1;
                    units.push((u as f32, format!("c:{}", t[i].path.display()), Source::Cover(t[i].path.clone())));
                    let a = t[i].album.clone();
                    while i > 0 && t[i - 1].album == a && t[i - 1].path.parent() == t[i].path.parent() {
                        i -= 1;
                    }
                }
            }
            let (mut u, mut i) = (unit, sel);
            let mut ahead = 0;
            while i + 1 < t.len() && ahead < 4 {
                i += 1;
                if t[i].album != t[i - 1].album || t[i].path.parent() != t[i - 1].path.parent() {
                    u += 1;
                    ahead += 1;
                    units.push((u as f32, format!("c:{}", t[i].path.display()), Source::Cover(t[i].path.clone())));
                }
            }
            return Some((format!("list:{}", third.title), unit as f32, units));
        }
        let lo = third.sel.saturating_sub(2);
        for (i, e) in third.entries.iter().enumerate().skip(lo).take(8) {
            let item = match &e.action {
                Action::Slideshow(paths) => paths.first().map(|p| {
                    let s = p.display().to_string();
                    if s.starts_with("http") { (format!("u:{s}"), Source::Url(s)) } else { (format!("w:{s}"), Source::File(p.clone())) }
                }),
                _ => e.thumb.clone(),
            };
            if let Some((k, s)) = item {
                units.push((i as f32, k, s));
            }
        }
        if units.is_empty() {
            return None;
        }
        Some((format!("list:{}", third.title), third.sel as f32, units))
    }

    /// Artwork (album cover, photo, video thumbnail) as tilted planes in the left half of the screen. The
    /// selected one is centred; the others fan out behind it, and the whole row glides as the selection moves.
    fn draw_wall(&mut self, now: f64, alpha: f32) -> bool {
        let Some((id, target, units)) = self.wall_plan() else { return false };
        if id != self.wall_id {
            self.wall_id = id;
            self.wall_pos.set(target);
            self.wall_fade.set(0.0);
            self.wall_fade.go(now, 1.0, 0.35);
        } else if (self.wall_pos.target() - target).abs() > 0.001 {
            self.wall_pos.go(now, target, 0.3);
        }
        let pos = self.wall_pos.value(now);
        let alpha = alpha * self.wall_fade.value(now);
        // (x offset from centre, width factor, height factor, opacity) at integer positions -2..=3
        const SLOTS: [(f32, f32, f32, f32); 6] = [
            (-420.0, 0.45, 0.70, 0.0),
            (-300.0, 0.55, 0.85, 0.0),
            (0.0, 1.0, 1.0, 1.0),
            (330.0, 0.36, 0.82, 0.92),
            (405.0, 0.26, 0.66, 0.6),
            (450.0, 0.2, 0.5, 0.0),
        ];
        let slot = |p: f32| -> (f32, f32, f32, f32) {
            let q = (p + 2.0).clamp(0.0, 5.0);
            let i = (q.floor() as usize).min(4);
            let f = q - i as f32;
            let (a, b) = (SLOTS[i], SLOTS[i + 1]);
            (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f, a.2 + (b.2 - a.2) * f, a.3 + (b.3 - a.3) * f)
        };
        let mut order: Vec<_> = units.iter().map(|(i, k, s)| (i - pos, k.clone(), s.clone())).filter(|(p, _, _)| *p > -2.0 && *p < 3.0).collect();
        order.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap_or(std::cmp::Ordering::Equal));
        let (cx, cy) = (470.0f32, 470.0f32);
        let mut drew = false;
        for (p, key, src) in order {
            let Some(Some(tex)) = self.tex.get(&key, src, 1024) else { continue };
            let tex = tex.clone();
            let aspect = (tex.width() / tex.height().max(1.0)).clamp(0.4, 2.4);
            let (maxw, maxh) = (560.0f32, 460.0f32);
            let base_h = (maxw / aspect).min(maxh);
            let base_w = base_h * aspect;
            let (off, wf, hf, a) = slot(p);
            if a <= 0.01 {
                continue;
            }
            let (w, h) = (base_w * wf, base_h * hf);
            let x_near = cx + off - w / 2.0;
            let far = if p.abs() < 0.5 { 0.9 } else { 0.86 };
            draw_perspective(&tex, x_near, h, x_near + w, h * far, cy, alpha * a);
            drew = true;
        }
        drew
    }
}

/// The YouTube logo (the supplied artwork), centred on (cx, cy).
pub fn draw_youtube_logo(g: &Gfx, cx: f32, cy: f32, alpha: f32) {
    let h = 62.0;
    let w = h * g.yt_logo.width() / g.yt_logo.height().max(1.0);
    draw_tex(&g.yt_logo, cx - w / 2.0, cy - h / 2.0, w, h, white(alpha));
}
