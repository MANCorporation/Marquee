//! The main menu's arc of section icons: a port of the Swift `RootCarouselLayerController`
//! slot maths. All coordinates are scene points, origin top left, y down.

use crate::anim::lerp;
use macroquad::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct CarouselConfig {
    pub selected_index: i32,
    pub is_icon_animated: bool,
    pub is_entering_submenu: bool,
    pub is_returning_to_root: bool,
    pub is_in_submenu: bool,
    pub icon_size: f32,
    pub scale_reduction: f32,
    pub landed_icon_scale: f32,
    pub selected_size_multiplier: f32,
    pub arc_radius: f32,
    pub resting_offset: Vec2,
    pub landed_offset: Vec2,
    pub entry_offset: f32,
    pub scene_width: f32,
    pub box_origin: Vec2,
}

#[derive(Clone, Copy, Debug)]
pub struct IconState {
    pub center: Vec2,
    pub size: f32,
    pub opacity: f32,
    pub blur_radius: f32,
    pub reflection_center: Vec2,
    pub reflection_size: f32,
    pub reflection_opacity: f32,
    pub reflection_blur: f32,
    pub shows_reflection: bool,
}

impl IconState {
    pub fn lerp(a: &IconState, b: &IconState, t: f32) -> IconState {
        IconState {
            center: a.center.lerp(b.center, t),
            size: lerp(a.size, b.size, t),
            opacity: lerp(a.opacity, b.opacity, t),
            blur_radius: lerp(a.blur_radius, b.blur_radius, t),
            reflection_center: a.reflection_center.lerp(b.reflection_center, t),
            reflection_size: lerp(a.reflection_size, b.reflection_size, t),
            reflection_opacity: lerp(a.reflection_opacity, b.reflection_opacity, t),
            reflection_blur: lerp(a.reflection_blur, b.reflection_blur, t),
            shows_reflection: if t < 0.5 { a.shows_reflection } else { b.shows_reflection },
        }
    }
}

/// The icon `position` places after the selection (negative: before it).
pub fn slot_state(position: i32, c: &CarouselConfig, bounds: Vec2) -> IconState {
    let angle = position as f32 * std::f32::consts::PI / 6.0;
    let is_landed = position == 0 && c.is_icon_animated;
    let is_exit_slot = position == -1;
    let abs_pos = position.abs();
    let scale = if is_landed {
        c.landed_icon_scale
    } else if is_exit_slot {
        1.0
    } else {
        1.0 - abs_pos as f32 * c.scale_reduction
    };
    let (mut x, y);
    if is_landed {
        x = c.landed_offset.x;
        y = c.landed_offset.y;
    } else if position == 0 {
        x = -1.25 * c.arc_radius * angle.sin() * 1.5 + c.resting_offset.x;
        y = c.resting_offset.y;
    } else if (1..=3).contains(&position) {
        x = -1.25 * c.arc_radius * angle.sin() * 0.9 - 0.1 * c.icon_size;
        y = -c.arc_radius * (1.0 - angle.cos()) * 0.025
            + if position < 2 { 0.2 * c.icon_size } else { 0.15 * c.icon_size };
    } else if is_exit_slot {
        let front_size = c.icon_size * c.selected_size_multiplier;
        let front = vec2(c.resting_offset.x - c.entry_offset, c.resting_offset.y + c.entry_offset);
        x = -front_size * 0.5 - 24.0 - c.box_origin.x - bounds.x / 2.0;
        y = front.y + (front.x - x) * ((1280.0 - front.y) / (front.x + 1920.0));
    } else {
        x = -1920.0;
        y = 1280.0;
    }
    if c.is_entering_submenu && position != 0 {
        x -= (c.scene_width * 1.15).max(c.icon_size * 3.0);
    }
    let base_opacity = if position == 0 || abs_pos <= 2 {
        1.0
    } else {
        (1.0 - abs_pos as f32 * 0.4).max(0.0)
    };
    let mut opacity = if c.is_entering_submenu && position != 0 {
        (base_opacity - 0.95).max(0.0)
    } else {
        base_opacity
    };
    let is_incoming = position == 0 && !c.is_icon_animated;
    let base_x = x + if is_incoming { -c.entry_offset } else { 0.0 };
    let base_y = y + if is_incoming { c.entry_offset } else { 0.0 };
    let adjusted_size = if !is_exit_slot && (abs_pos == 1 || abs_pos == 2) {
        c.icon_size
    } else {
        c.icon_size * c.selected_size_multiplier
    };
    let settled = c.is_in_submenu && !c.is_entering_submenu && !c.is_returning_to_root;
    if settled && position != 0 {
        opacity = 0.0;
    }
    let animate_reflection = position == 0 && (!c.is_in_submenu || c.is_entering_submenu || c.is_returning_to_root);
    let detached = animate_reflection
        && ((c.is_entering_submenu && !c.is_returning_to_root) || (c.is_returning_to_root && c.is_icon_animated));
    let compensation = if position == 0 && (c.is_icon_animated || c.is_entering_submenu) {
        vec2(x - c.resting_offset.x, y - c.resting_offset.y)
    } else {
        Vec2::ZERO
    };
    let reflection_offset = if detached {
        vec2(90.0 - compensation.x, 920.0 - compensation.y)
    } else {
        vec2(0.0, -(adjusted_size * 0.215))
    };
    let reflection_scale = if detached { 1.0 } else { scale };
    let shows_reflection = !(position == 0 && settled);
    let center = vec2(bounds.x / 2.0 + base_x, bounds.y / 2.0 - 50.0 + base_y);
    IconState {
        center,
        size: adjusted_size * scale,
        opacity,
        blur_radius: if position != 0 && !is_exit_slot { 10.0 } else { 0.0 },
        reflection_center: vec2(
            reflection_offset.x * reflection_scale,
            (adjusted_size + reflection_offset.y) * reflection_scale,
        ),
        reflection_size: adjusted_size * reflection_scale,
        reflection_opacity: if detached { 0.22 } else { 0.34 },
        reflection_blur: if detached { 3.0 } else { 0.0 },
        shows_reflection,
    }
}

/// Where icon `index` is at the continuous selection `phase`.
pub fn state_at_phase(index: usize, phase: f32, c: &CarouselConfig, bounds: Vec2) -> IconState {
    let position = index as f32 - phase;
    let lower = position.floor();
    let fraction = position - lower;
    let a = slot_state(lower as i32, c, bounds);
    if fraction < 0.0001 {
        return a;
    }
    let b = slot_state(lower as i32 + 1, c, bounds);
    IconState::lerp(&a, &b, fraction)
}

/// Back-to-front draw order around the selection.
pub fn draw_order(count: usize, selected: i32, animated: bool) -> Vec<usize> {
    let n = count as i32;
    let mut order: Vec<usize> = (0..n).rev().map(|i| ((selected + i).rem_euclid(n)) as usize).collect();
    let z = |idx: usize| {
        let pos = idx as i32 - selected;
        if pos == 0 {
            if animated { 1000 } else { 1 }
        } else if (1..=3).contains(&pos) {
            0
        } else {
            1000
        }
    };
    order.sort_by_key(|&i| z(i));
    order
}
