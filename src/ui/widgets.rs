use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, SQRT_2};

use crate::model::key::Key;
use crate::protocol::layout::get_key_layout;
use crate::ui::{draw::DrawList, text::FontAtlas, input::{self, InputState}, theme as t};

// ═══════════════════════════════════════════════════════════════
//  Card helper
// ═══════════════════════════════════════════════════════════════

pub fn card(dl: &mut DrawList, x: f32, y: f32, w: f32, h: f32) {
    // Drop shadow
    dl.rounded_rect(x, y + 3.0, w, h, t::CARD_R + 2.0, [0.0, 0.0, 0.0, 0.18]);
    // Fill
    dl.rounded_rect(x, y, w, h, t::CARD_R, t::BG_SURFACE);
    // Subtle 1px border
    dl.rounded_rect(x, y, w, h, t::CARD_R, t::CARD_BORDER);
    dl.rounded_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, (t::CARD_R - 1.0).max(0.0), t::BG_SURFACE);
}

pub fn mini_card(dl: &mut DrawList, x: f32, y: f32, w: f32, h: f32) {
    dl.rounded_rect(x, y, w, h, t::MINI_CARD_R, t::BG_RAISED);
    // Subtle inset border so the card reads against similar backgrounds
    dl.rounded_rect(x, y, w, h, t::MINI_CARD_R, [1.0, 1.0, 1.0, 0.03]);
    dl.rounded_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, (t::MINI_CARD_R - 1.0).max(0.0), t::BG_RAISED);
}

// ═══════════════════════════════════════════════════════════════
//  Slider
// ═══════════════════════════════════════════════════════════════

#[derive(Default)]
pub struct SliderState { pub dragging: bool, visual_frac: f32, hover_t: f32 }

pub fn slider(
    dl: &mut DrawList, fa: &FontAtlas, inp: &InputState,
    state: &mut SliderState,
    x: f32, y: f32, w: f32,
    val: &mut f64, min: f64, max: f64, step: f64,
    label: &str, label_col: [f32; 4],
) -> bool {
    let h = 28.0;
    let track_h = 5.0;
    let thumb_r = 7.0;

    fa.draw_text(dl, label, x, y + 2.0, t::FONT_SMALL, label_col);
    let lw = fa.measure(label, t::FONT_SMALL).0 + 8.0;
    let sx = x + lw;
    let sw = w - lw - 60.0;
    let ty = y + h * 0.5;

    // Track background with subtle border
    dl.rounded_rect(sx, ty - track_h * 0.5, sw, track_h, track_h * 0.5, t::SLIDER_TRACK);
    dl.rounded_rect(sx, ty - track_h * 0.5, sw, track_h, track_h * 0.5, t::SLIDER_TRACK_BORDER);
    dl.rounded_rect(
        sx + 0.5, ty - track_h * 0.5 + 0.5,
        sw - 1.0, track_h - 1.0,
        (track_h - 1.0) * 0.5, t::SLIDER_TRACK,
    );

    let target = ((*val - min) / (max - min)).clamp(0.0, 1.0) as f32;
    state.visual_frac = input::smooth(state.visual_frac, target, t::ANIM_SLOW, inp.dt);
    let thumb_x = sx + state.visual_frac * sw;

    // Filled portion
    if state.visual_frac > 0.005 {
        dl.rounded_rect(sx, ty - track_h * 0.5, thumb_x - sx, track_h, track_h * 0.5, t::ACCENT);
    }

    // Hover
    let hovered = inp.in_rect(sx - thumb_r, y, sw + thumb_r * 2.0, h);
    state.hover_t = input::smooth(
        state.hover_t,
        if hovered || state.dragging { 1.0 } else { 0.0 },
        t::ANIM_FAST, inp.dt,
    );

    let tr = thumb_r + state.hover_t * 1.5;

    // Focus ring (accent glow) when dragging
    if state.dragging {
        dl.circle(thumb_x, ty, tr + 5.0, t::with_alpha(t::ACCENT, 0.25));
    } else if state.hover_t > 0.01 {
        dl.circle(thumb_x, ty, tr + 3.0, t::with_alpha(t::ACCENT, 0.12 * state.hover_t));
    }

    // Thumb shadow
    dl.circle(thumb_x, ty + 1.0, tr + 0.5, [0.0, 0.0, 0.0, 0.22]);
    // Thumb
    let tc = t::lerp4([0.82, 0.82, 0.82, 1.0], t::TEXT, state.hover_t);
    dl.circle(thumb_x, ty, tr, tc);

    // Value text
    let txt = format!("{:.2} mm", val);
    fa.draw_text(dl, &txt, x + w - 55.0, y + 6.0, t::FONT_SMALL, t::TEXT_SEC);

    // Interaction
    if inp.just_pressed() && hovered { state.dragging = true; }
    if !inp.mouse_down { state.dragging = false; }

    let mut changed = false;
    if state.dragging {
        let frac = ((inp.mouse_x - sx) / sw).clamp(0.0, 1.0) as f64;
        let q = ((min + frac * (max - min)) / step).round() * step;
        if (*val - q).abs() > 1e-9 { *val = q; changed = true; }
    }
    changed
}

// ═══════════════════════════════════════════════════════════════
//  Button
// ═══════════════════════════════════════════════════════════════

#[derive(Default)]
pub struct ButtonAnim { anims: HashMap<u64, f32> }

impl ButtonAnim {
    fn ht(&mut self, x: f32, y: f32, hov: bool, dt: f32) -> f32 {
        let k = ((x as u32 as u64) << 32) | (y as u32 as u64);
        let v = self.anims.entry(k).or_insert(0.0);
        *v = input::smooth(*v, if hov { 1.0 } else { 0.0 }, t::ANIM_FAST, dt);
        *v
    }
}

pub fn button_anim(
    dl: &mut DrawList, fa: &FontAtlas, inp: &InputState,
    anims: &mut ButtonAnim,
    x: f32, y: f32, w: f32, h: f32, label: &str, bg: [f32; 4], enabled: bool,
) -> bool {
    let hov = enabled && inp.in_rect(x, y, w, h);
    let pressed = hov && inp.mouse_down;
    let ht = anims.ht(x, y, hov, inp.dt);
    let col = if pressed { t::lerp4(bg, [0.0, 0.0, 0.0, 1.0], 0.25) }
              else { t::lerp4(bg, t::lerp4(bg, [1.0; 4], 0.12), ht) };
    let tc = if enabled { t::lerp4(t::TEXT_SEC, t::TEXT, ht) } else { t::TEXT_DIM };
    dl.rounded_rect(x, y + 1.0, w, h, 6.0, [0.0, 0.0, 0.0, 0.12 * ht]);
    dl.rounded_rect(x, y, w, h, 6.0, col);
    if ht > 0.01 {
        dl.rounded_rect(x, y, w, h, 6.0, t::with_alpha(t::BORDER, ht * 0.5));
        dl.rounded_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, 5.0, col);
    }
    fa.draw_centered(dl, label, x + w * 0.5, y + h * 0.5, t::FONT_NORMAL, tc);
    hov && inp.clicked()
}

pub fn button(
    dl: &mut DrawList, fa: &FontAtlas, inp: &InputState,
    x: f32, y: f32, w: f32, h: f32, label: &str, bg: [f32; 4], enabled: bool,
) -> bool {
    let hov = enabled && inp.in_rect(x, y, w, h);
    let pressed = hov && inp.mouse_down;
    let col = if pressed { t::lerp4(bg, [0.0;4], 0.2) }
              else if hov { t::lerp4(bg, [1.0;4], 0.1) } else { bg };
    let tc = if enabled { t::TEXT } else { t::TEXT_DIM };
    dl.rounded_rect(x, y, w, h, 6.0, col);
    fa.draw_centered(dl, label, x + w * 0.5, y + h * 0.5, t::FONT_NORMAL, tc);
    hov && inp.clicked()
}

// ═══════════════════════════════════════════════════════════════
//  Toggle — Adwaita-style, always visible
// ═══════════════════════════════════════════════════════════════

#[derive(Default)]
pub struct ToggleState { anim_t: f32, hover_t: f32 }

pub fn toggle_anim(
    dl: &mut DrawList, inp: &InputState,
    state: &mut ToggleState, x: f32, y: f32, val: &mut bool,
) -> bool {
    let w = 40.0;
    let h = 22.0;
    let r = h * 0.5;
    let thumb_r = r - 3.0;

    // Smooth ON/OFF transition
    state.anim_t = input::smooth(
        state.anim_t,
        if *val { 1.0 } else { 0.0 },
        t::ANIM_NORMAL, inp.dt,
    );

    let hov = inp.in_rect(x - 2.0, y - 2.0, w + 4.0, h + 4.0);
    state.hover_t = input::smooth(
        state.hover_t,
        if hov { 1.0 } else { 0.0 },
        t::ANIM_FAST, inp.dt,
    );

    // Track colour: OFF = dark visible track, ON = accent
    let track_bg = t::lerp4(t::TOGGLE_OFF, t::ACCENT, state.anim_t);
    // Brighten slightly on hover
    let track_col = t::lerp4(track_bg, t::lerp4(track_bg, [1.0; 4], 0.08), state.hover_t);

    // Track shadow
    dl.rounded_rect(x, y + 1.0, w, h, r, [0.0, 0.0, 0.0, 0.15]);
    // Track fill
    dl.rounded_rect(x, y, w, h, r, track_col);
    // Track border — always visible, stronger when OFF
    let border_a = 0.14 - state.anim_t * 0.06;
    dl.rounded_rect(x, y, w, h, r, [1.0, 1.0, 1.0, border_a]);
    dl.rounded_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, r - 1.0, track_col);

    // Thumb position
    let tx = x + r + (w - h) * state.anim_t;
    let ty = y + r;

    // Thumb shadow
    dl.circle(tx, ty + 1.0, thumb_r + 0.5, [0.0, 0.0, 0.0, 0.25]);
    // Thumb fill
    dl.circle(tx, ty, thumb_r, t::TEXT);
    // Thumb highlight (subtle specular dot at top)
    dl.circle(tx, ty - thumb_r * 0.35, thumb_r * 0.3, [1.0, 1.0, 1.0, 0.15]);

    let clicked = inp.clicked() && hov;
    if clicked { *val = !*val; }
    clicked
}

// ═══════════════════════════════════════════════════════════════
//  Keyboard Grid
// ═══════════════════════════════════════════════════════════════

pub struct HoveredKey { pub code: usize, pub name: &'static str, pub x: f32, pub y: f32, pub w: f32 }
pub struct GridResult { pub height: f32, pub hovered: Option<HoveredKey>, pub right_click: Option<(usize, f32, f32)> }

#[derive(Default)]
pub struct GridState {
    pub selected: HashSet<usize>,
    adding: bool,
    dragging: bool,
    hover_t: HashMap<usize, f32>,
    select_t: HashMap<usize, f32>,
}

impl GridState {
    pub fn select_all(&mut self, kl: &[Option<&str>]) {
        if self.selected.is_empty() {
            for (i, e) in kl.iter().enumerate() { if e.is_some() { self.selected.insert(i); } }
        } else { self.selected.clear(); }
    }
}

pub fn keyboard_grid(
    dl: &mut DrawList, fa: &FontAtlas, inp: &InputState,
    keys: &[Option<Key>], state: &mut GridState, show_act: bool,
    key_list: &[Option<&str>], x0: f32, y0: f32, avail_w: f32,
) -> GridResult {
    let cols = 16.0f32;
    let unit = (avail_w / cols).min(t::UNIT_PX);
    let row_h = unit * 0.84;
    let gap = t::KEY_GAP;
    let xoff = (avail_w - cols * unit) * 0.5;

    struct RK { code: usize, name: &'static str, rx: f32, ry: f32, rw: f32, rh: f32 }
    let mut rks = Vec::with_capacity(70);
    for (idx, entry) in key_list.iter().enumerate() {
        if let Some(name) = entry {
            if let Some(l) = get_key_layout(name) {
                rks.push(RK { code: idx, name: l.name,
                    rx: x0 + xoff + l.column * unit + gap * 0.5,
                    ry: y0 + l.row as f32 * (row_h + gap),
                    rw: l.width * unit - gap, rh: row_h });
            }
        }
    }

    // Input
    if inp.just_pressed() {
        for rk in &rks {
            if inp.in_rect(rk.rx, rk.ry, rk.rw, rk.rh) {
                state.adding = !state.selected.contains(&rk.code);
                state.dragging = true;
                break;
            }
        }
    }
    if !inp.mouse_down { state.dragging = false; }

    let mut result = GridResult { height: 5.0 * (row_h + gap), hovered: None, right_click: None };

    for rk in &rks {
        let hov = inp.in_rect(rk.rx, rk.ry, rk.rw, rk.rh);
        if state.dragging && hov {
            if state.adding { state.selected.insert(rk.code); }
            else { state.selected.remove(&rk.code); }
        }
        let is_sel = state.selected.contains(&rk.code);

        let ht = state.hover_t.entry(rk.code).or_insert(0.0);
        *ht = input::smooth(*ht, if hov { 1.0 } else { 0.0 }, t::ANIM_FAST, inp.dt);
        let hover_t = *ht;

        let st = state.select_t.entry(rk.code).or_insert(0.0);
        *st = input::smooth(*st, if is_sel { 1.0 } else { 0.0 }, t::ANIM_NORMAL, inp.dt);
        let select_t = *st;

        let pressed = inp.mouse_down && hov;
        let press_dy = if pressed { 1.0 } else { 0.0 };

        let base = match keys.get(rk.code) {
            Some(Some(k)) if k.color.r > 0 || k.color.g > 0 || k.color.b > 0 =>
                t::dim_key_color(k.color.r, k.color.g, k.color.b),
            _ => t::BG_KEY,
        };
        let bg = t::lerp4(t::lerp4(base, t::BG_KEY_HOVER, hover_t), t::BG_KEY_ACTIVE, select_t * 0.4);

        // Shadow (shrinks when pressed)
        let shadow_a = (0.20 + hover_t * 0.15) * (1.0 - press_dy * 0.5);
        dl.rounded_rect(rk.rx, rk.ry + 1.5 + press_dy, rk.rw, rk.rh, t::KEY_R, [0.0, 0.0, 0.0, shadow_a]);

        let ky = rk.ry + press_dy;

        // Selection glow
        if select_t > 0.01 {
            let e = 3.0 * select_t;
            dl.rounded_rect(rk.rx - e, ky - e, rk.rw + e * 2.0, rk.rh + e * 2.0,
                t::KEY_R + e, t::with_alpha(t::ACCENT, 0.15 * select_t));
        }

        // Fill
        dl.rounded_rect(rk.rx, ky, rk.rw, rk.rh, t::KEY_R, bg);

        // Top bevel
        dl.rect(rk.rx + 2.0, ky, rk.rw - 4.0, 1.0, [1.0, 1.0, 1.0, 0.04 + hover_t * 0.04]);

        // Selection border
        if select_t > 0.01 {
            let bw = 2.0;
            dl.rounded_rect(rk.rx, ky, rk.rw, rk.rh, t::KEY_R, t::with_alpha(t::ACCENT, 0.7 * select_t));
            dl.rounded_rect(rk.rx + bw, ky + bw, rk.rw - bw * 2.0, rk.rh - bw * 2.0,
                (t::KEY_R - bw).max(0.0), bg);
        }

        // Label
        let lb = (hover_t * 0.5 + select_t * 0.5).min(1.0);
        fa.draw_centered(dl, rk.name, rk.rx + rk.rw * 0.5, ky + rk.rh * 0.5, t::FONT_KEY,
            t::lerp4(t::TEXT_SEC, t::TEXT, lb));

        // Actuation overlays
        if show_act {
            if let Some(Some(k)) = keys.get(rk.code) {
                if k.down_actuation > 0.0 {
                    let s = format!("{:.2}", k.down_actuation);
                    let tw = fa.measure(&s, t::FONT_TINY).0;
                    fa.draw_text(dl, &s, rk.rx + (rk.rw - tw) * 0.5, ky + rk.rh - 12.0, t::FONT_TINY, t::PRESS_COL);
                }
                if k.up_actuation > 0.0 {
                    let s = format!("{:.2}", k.up_actuation);
                    let tw = fa.measure(&s, t::FONT_TINY).0;
                    fa.draw_text(dl, &s, rk.rx + (rk.rw - tw) * 0.5, ky + 2.0, t::FONT_TINY, t::RELEASE_COL);
                }
            }
        }

        // Tooltip info
        if hover_t > 0.5 {
            result.hovered = Some(HoveredKey { code: rk.code, name: rk.name,
                x: rk.rx + rk.rw * 0.5, y: rk.ry, w: rk.rw });
        }
        // Context menu
        if inp.right_clicked() && hov {
            result.right_click = Some((rk.code, inp.mouse_x, inp.mouse_y));
        }
    }

    result
}

// ═══════════════════════════════════════════════════════════════
//  HSV Colour Wheel — high-segment, smooth
// ═══════════════════════════════════════════════════════════════

#[derive(Default)]
pub struct HsvPickerState {
    pub hue: f32, pub sat: f32, pub val: f32,
    pub dragging_ring: bool, pub dragging_sv: bool,
}

impl HsvPickerState {
    pub fn is_dragging(&self) -> bool { self.dragging_ring || self.dragging_sv }
    pub fn sync_from_rgb(&mut self, r: u8, g: u8, b: u8) {
        let (h, s, v) = t::rgb_to_hsv(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
        self.hue = h; self.sat = s; self.val = v;
    }
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        let [r, g, b] = t::hsv_to_rgb(self.hue, self.sat, self.val);
        ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
    }
}

pub fn hsv_picker(
    dl: &mut DrawList, _fa: &FontAtlas, inp: &InputState,
    state: &mut HsvPickerState,
    cx: f32, cy: f32, radius: f32,
) -> bool {
    let r_out = radius;
    let ring_w = 12.0;
    let r_in = r_out - ring_w;

    // Number of segments — 180 = imperceptibly smooth hue ring
    let hue_seg = 180u32;
    // SV square subdivisions
    let sv_div = 28u32;

    // ── Dark backdrop behind the ring for depth ────────────────
    dl.circle(cx, cy, r_out + 1.0, [0.0, 0.0, 0.0, 0.20]);

    // ── SV square inside the ring ──────────────────────────────
    let half = r_in * 0.70;  // leave a tiny gap between square and ring
    let sq_x = cx - half;
    let sq_y = cy - half;
    let sq_size = 2.0 * half;
    let step = sq_size / sv_div as f32;

    for i in 0..sv_div {
        for j in 0..sv_div {
            let s0 = i as f32 / sv_div as f32;
            let s1 = (i + 1) as f32 / sv_div as f32;
            let v0 = 1.0 - j as f32 / sv_div as f32;
            let v1 = 1.0 - (j + 1) as f32 / sv_div as f32;
            let c00 = t::hsv_to_rgba(state.hue, s0, v0);
            let c10 = t::hsv_to_rgba(state.hue, s1, v0);
            let c01 = t::hsv_to_rgba(state.hue, s0, v1);
            let c11 = t::hsv_to_rgba(state.hue, s1, v1);
            dl.quad_colors(
                [sq_x + i as f32 * step,       sq_y + j as f32 * step],       c00,
                [sq_x + (i + 1) as f32 * step, sq_y + j as f32 * step],       c10,
                [sq_x + (i + 1) as f32 * step, sq_y + (j + 1) as f32 * step], c11,
                [sq_x + i as f32 * step,       sq_y + (j + 1) as f32 * step], c01,
            );
        }
    }

    // SV square subtle border
    let bdr = [1.0, 1.0, 1.0, 0.06];
    dl.rect(sq_x, sq_y, sq_size, 1.0, bdr);
    dl.rect(sq_x, sq_y + sq_size - 1.0, sq_size, 1.0, bdr);
    dl.rect(sq_x, sq_y, 1.0, sq_size, bdr);
    dl.rect(sq_x + sq_size - 1.0, sq_y, 1.0, sq_size, bdr);

    // ── Hue ring ───────────────────────────────────────────────
    for i in 0..hue_seg {
        let a0 = 2.0 * PI * i as f32 / hue_seg as f32;
        let a1 = 2.0 * PI * (i + 1) as f32 / hue_seg as f32;
        let h0 = 360.0 * i as f32 / hue_seg as f32;
        let h1 = 360.0 * (i + 1) as f32 / hue_seg as f32;
        let c0 = t::hsv_to_rgba(h0, 1.0, 1.0);
        let c1 = t::hsv_to_rgba(h1, 1.0, 1.0);
        let (sin0, cos0) = a0.sin_cos();
        let (sin1, cos1) = a1.sin_cos();
        dl.ring_segment(
            [cx + r_out * cos0, cy + r_out * sin0],
            [cx + r_out * cos1, cy + r_out * sin1],
            [cx + r_in * cos0,  cy + r_in * sin0],
            [cx + r_in * cos1,  cy + r_in * sin1],
            c0, c1,
        );
    }

    // Soft edge rings for cleaner outer/inner boundary
    dl.ring(cx, cy, r_out, r_out + 1.5, 96, [t::BG_RAISED[0], t::BG_RAISED[1], t::BG_RAISED[2], 0.7]);
    dl.ring(cx, cy, r_in - 1.5, r_in, 96, [t::BG_RAISED[0], t::BG_RAISED[1], t::BG_RAISED[2], 0.5]);

    // ── Hue indicator ──────────────────────────────────────────
    let ha = state.hue.to_radians();
    let r_mid = (r_out + r_in) * 0.5;
    let hx = cx + r_mid * ha.cos();
    let hy = cy + r_mid * ha.sin();
    dl.circle(hx, hy + 1.0, 6.0, [0.0, 0.0, 0.0, 0.35]);      // shadow
    dl.circle(hx, hy, 5.5, [1.0, 1.0, 1.0, 1.0]);              // outer ring
    dl.circle(hx, hy, 3.5, t::hsv_to_rgba(state.hue, 1.0, 1.0)); // hue colour fill

    // ── SV indicator ───────────────────────────────────────────
    let sx = sq_x + state.sat * sq_size;
    let sy = sq_y + (1.0 - state.val) * sq_size;
    dl.circle(sx, sy + 1.0, 6.0, [0.0, 0.0, 0.0, 0.35]);
    dl.circle(sx, sy, 5.5, [1.0, 1.0, 1.0, 1.0]);
    dl.circle(sx, sy, 3.5, t::hsv_to_rgba(state.hue, state.sat, state.val));

    // ── Interaction ────────────────────────────────────────────
    let dx = inp.mouse_x - cx;
    let dy = inp.mouse_y - cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if inp.just_pressed() {
        if dist >= r_in - 2.0 && dist <= r_out + 4.0 {
            state.dragging_ring = true;
        } else if (inp.mouse_x - cx).abs() <= half + 3.0
               && (inp.mouse_y - cy).abs() <= half + 3.0
               && dist < r_in + 2.0 {
            state.dragging_sv = true;
        }
    }
    if !inp.mouse_down { state.dragging_ring = false; state.dragging_sv = false; }

    let mut changed = false;
    if state.dragging_ring {
        let a = dy.atan2(dx).to_degrees();
        state.hue = if a < 0.0 { a + 360.0 } else { a };
        changed = true;
    }
    if state.dragging_sv {
        state.sat = ((inp.mouse_x - sq_x) / sq_size).clamp(0.0, 1.0);
        state.val = 1.0 - ((inp.mouse_y - sq_y) / sq_size).clamp(0.0, 1.0);
        changed = true;
    }
    changed
}

// ═══════════════════════════════════════════════════════════════
//  Colour presets
// ═══════════════════════════════════════════════════════════════

pub fn color_presets(
    dl: &mut DrawList, fa: &FontAtlas, inp: &InputState,
    keys: &mut [Option<Key>], sel: &HashSet<usize>, x: f32, y: f32,
) -> bool {
    let presets: &[(&str, [u8; 3])] = &[
        ("R", [255,0,0]), ("G", [0,255,0]), ("B", [0,0,255]),
        ("W", [255,255,255]), ("Off", [0,0,0]),
    ];
    let mut changed = false;
    let mut cx = x;
    for (label, rgb) in presets {
        let bw = fa.measure(label, t::FONT_SMALL).0 + 14.0;
        let col = t::rgb_to_col(rgb[0].max(30), rgb[1].max(30), rgb[2].max(30));
        if button(dl, fa, inp, cx, y, bw, 22.0, label, col, true) {
            let nc = crate::model::key::KeyColor { r: rgb[0], g: rgb[1], b: rgb[2] };
            for &c in sel { if let Some(Some(k)) = keys.get_mut(c) { k.color = nc; } }
            changed = true;
        }
        cx += bw + 4.0;
    }
    changed
}

// ═══════════════════════════════════════════════════════════════
//  Tooltip
// ═══════════════════════════════════════════════════════════════

pub fn draw_tooltip(dl: &mut DrawList, fa: &FontAtlas, keys: &[Option<Key>], hk: &HoveredKey) {
    let k = match keys.get(hk.code) { Some(Some(k)) => k, _ => return };
    let line1 = hk.name;
    let line2 = format!("Press: {:.2}mm  Rel: {:.2}mm", k.down_actuation, k.up_actuation);
    let tw = fa.measure(line1, t::FONT_SMALL).0.max(fa.measure(&line2, t::FONT_SMALL).0) + 24.0;
    let th = 42.0;
    let tx = (hk.x - tw * 0.5).max(4.0);
    let ty = hk.y - th - 6.0;
    dl.rounded_rect(tx + 2.0, ty + 2.0, tw, th, 8.0, [0.0, 0.0, 0.0, 0.25]);
    dl.rounded_rect(tx, ty, tw, th, 8.0, t::BG_SURFACE);
    dl.rounded_rect(tx, ty, tw, th, 8.0, t::CARD_BORDER);
    dl.rounded_rect(tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, 7.0, t::BG_SURFACE);
    fa.draw_text(dl, line1, tx + 10.0, ty + 5.0, t::FONT_SMALL, t::TEXT);
    fa.draw_text(dl, &line2, tx + 10.0, ty + 22.0, t::FONT_SMALL, t::TEXT_SEC);
    let sc = t::rgb_to_col(k.color.r, k.color.g, k.color.b);
    dl.rounded_rect(tx + tw - 20.0, ty + 6.0, 12.0, 12.0, 3.0, sc);
}