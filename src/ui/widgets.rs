/// Reusable UI widgets for the AK680 MAX driver application.
///
/// Every widget follows the immediate-mode pattern: it takes the
/// current `DrawList`, `FontAtlas`, and `InputState`, draws itself,
/// and returns whether the user interacted with it.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

use crate::model::key::Key;
use crate::protocol::layout::get_key_layout;
use crate::ui::{
    draw::DrawList,
    input::{self, InputState},
    text::FontAtlas,
    theme as t,
};

// Cards

/// Draw a raised card surface with shadow and border.
pub fn card(dl: &mut DrawList, x: f32, y: f32, w: f32, h: f32) {
    // Soft drop shadow
    dl.aa_rounded_rect(
        x + 1.0, y + 3.0, w, h,
        t::CARD_R + 1.0,
        [0.0, 0.0, 0.0, 0.14],
    );
    // Border (drawn as a slightly larger background)
    dl.aa_rounded_rect(x, y, w, h, t::CARD_R, t::CARD_BORDER);
    // Surface fill
    dl.aa_rounded_rect(
        x + 1.0, y + 1.0, w - 2.0, h - 2.0,
        (t::CARD_R - 1.0).max(0.0),
        t::BG_SURFACE,
    );
}

/// Draw a small inset card used for column groupings inside the
/// options panel.
pub fn mini_card(dl: &mut DrawList, x: f32, y: f32, w: f32, h: f32) {
    dl.aa_rounded_rect(x, y, w, h, t::MINI_CARD_R, [1.0, 1.0, 1.0, 0.04]);
    dl.aa_rounded_rect(
        x + 1.0, y + 1.0, w - 2.0, h - 2.0,
        (t::MINI_CARD_R - 1.0).max(0.0),
        t::BG_RAISED,
    );
}

// Slider

/// Per-slider animation state.
#[derive(Default)]
pub struct SliderState {
    pub dragging: bool,
    visual_frac: f32,
    hover_t: f32,
}

/// Draw an Adwaita-style horizontal slider.
///
/// `value_text` is the already-formatted string shown to the right
/// of the track (e.g. `"1.20 mm"` or `"5"`).
///
/// Returns `true` when the value changed this frame.
pub fn slider(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    state: &mut SliderState,
    x: f32,
    y: f32,
    w: f32,
    val: &mut f64,
    min: f64,
    max: f64,
    step: f64,
    label: &str,
    label_col: [f32; 4],
    value_text: &str,
) -> bool {
    let h = 28.0;
    let track_h = 6.0;
    let thumb_r = 7.5;
    let value_area = 68.0;

    // Label
    fa.draw_text(dl, label, x, y + (h - 12.0) * 0.5, t::FONT_SMALL, label_col);
    let lw = fa.measure(label, t::FONT_SMALL).0 + 8.0;

    let track_x = x + lw;
    let track_w = (w - lw - value_area).max(20.0);
    let track_cy = y + h * 0.5;

    // Track background
    dl.aa_rounded_rect(
        track_x,
        track_cy - track_h * 0.5,
        track_w,
        track_h,
        track_h * 0.5,
        t::SLIDER_TRACK,
    );

    // Animated fraction
    let target = ((*val - min) / (max - min)).clamp(0.0, 1.0) as f32;
    state.visual_frac = input::smooth(state.visual_frac, target, t::ANIM_SLOW, inp.dt);

    let fill_w = (state.visual_frac * track_w).clamp(0.0, track_w);
    let thumb_x = track_x + fill_w;

    // Filled portion (radius clamped so narrow fills look correct)
    if fill_w > 1.0 {
        let fill_r = (track_h * 0.5).min(fill_w * 0.5);
        dl.aa_rounded_rect(
            track_x,
            track_cy - track_h * 0.5,
            fill_w,
            track_h,
            fill_r,
            t::ACCENT,
        );
    }

    // Hover detection
    let hovered = inp.in_rect(track_x - thumb_r, y, track_w + thumb_r * 2.0, h);
    state.hover_t = input::smooth(
        state.hover_t,
        if hovered || state.dragging { 1.0 } else { 0.0 },
        t::ANIM_FAST,
        inp.dt,
    );

    let tr = thumb_r + state.hover_t * 1.5;

    // Focus glow
    if state.dragging {
        dl.aa_circle(thumb_x, track_cy, tr + 6.0, t::with_alpha(t::ACCENT, 0.20));
    } else if state.hover_t > 0.01 {
        dl.aa_circle(
            thumb_x,
            track_cy,
            tr + 4.0,
            t::with_alpha(t::ACCENT, 0.10 * state.hover_t),
        );
    }

    // Thumb shadow
    dl.aa_circle(thumb_x, track_cy + 1.0, tr + 0.5, [0.0, 0.0, 0.0, 0.20]);
    // Thumb
    let tc = t::lerp4([0.85, 0.85, 0.85, 1.0], t::TEXT, state.hover_t);
    dl.aa_circle(thumb_x, track_cy, tr, tc);

    // Value text (right-aligned)
    let vx = x + w - value_area;
    let vw = fa.measure(value_text, t::FONT_BODY).0;
    fa.draw_text(
        dl,
        value_text,
        vx + value_area - vw,
        y + (h - 13.0) * 0.5,
        t::FONT_BODY,
        t::TEXT_SEC,
    );

    // Interaction
    if inp.just_pressed() && hovered {
        state.dragging = true;
    }
    if !inp.mouse_down {
        state.dragging = false;
    }

    let mut changed = false;
    if state.dragging {
        let frac = ((inp.mouse_x - track_x) / track_w).clamp(0.0, 1.0) as f64;
        let q = ((min + frac * (max - min)) / step).round() * step;
        if (*val - q).abs() > 1e-9 {
            *val = q;
            changed = true;
        }
    }
    changed
}

// Button

/// Shared hover-animation cache for buttons.
#[derive(Default)]
pub struct ButtonAnim {
    anims: HashMap<u64, f32>,
}

impl ButtonAnim {
    fn ht(&mut self, x: f32, y: f32, hov: bool, dt: f32) -> f32 {
        let k = ((x as u32 as u64) << 32) | (y as u32 as u64);
        let v = self.anims.entry(k).or_insert(0.0);
        *v = input::smooth(*v, if hov { 1.0 } else { 0.0 }, t::ANIM_FAST, dt);
        *v
    }
}

/// Animated button with hover/press feedback.
///
/// Returns `true` on click.
pub fn button_anim(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    anims: &mut ButtonAnim,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    bg: [f32; 4],
    enabled: bool,
) -> bool {
    let hov = enabled && inp.in_rect(x, y, w, h);
    let pressed = hov && inp.mouse_down;
    let ht = anims.ht(x, y, hov, inp.dt);

    let col = if pressed {
        t::lerp4(bg, [0.0, 0.0, 0.0, 1.0], 0.25)
    } else {
        t::lerp4(bg, t::lerp4(bg, [1.0; 4], 0.12), ht)
    };
    let tc = if enabled {
        t::lerp4(t::TEXT_SEC, t::TEXT, ht)
    } else {
        t::TEXT_DIM
    };

    // Shadow
    if ht > 0.01 {
        dl.aa_rounded_rect(
            x, y + 1.0, w, h, 6.0,
            [0.0, 0.0, 0.0, 0.10 * ht],
        );
    }
    dl.aa_rounded_rect(x, y, w, h, 6.0, col);
    // Border on hover
    if ht > 0.01 {
        dl.aa_rounded_rect(x, y, w, h, 6.0, t::with_alpha(t::BORDER, ht * 0.5));
        dl.aa_rounded_rect(
            x + 1.0, y + 1.0, w - 2.0, h - 2.0, 5.0, col,
        );
    }
    fa.draw_centered(dl, label, x + w * 0.5, y + h * 0.5, t::FONT_NORMAL, tc);
    hov && inp.clicked()
}

/// Simple non-animated button.
pub fn button(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: &str,
    bg: [f32; 4],
    enabled: bool,
) -> bool {
    let hov = enabled && inp.in_rect(x, y, w, h);
    let pressed = hov && inp.mouse_down;
    let col = if pressed {
        t::lerp4(bg, [0.0; 4], 0.2)
    } else if hov {
        t::lerp4(bg, [1.0; 4], 0.1)
    } else {
        bg
    };
    let tc = if enabled { t::TEXT } else { t::TEXT_DIM };
    dl.aa_rounded_rect(x, y, w, h, 6.0, col);
    fa.draw_centered(dl, label, x + w * 0.5, y + h * 0.5, t::FONT_NORMAL, tc);
    hov && inp.clicked()
}

// Toggle (Adwaita switch)

/// Per-toggle animation state.
#[derive(Default)]
pub struct ToggleState {
    anim_t: f32,
    hover_t: f32,
}

/// Adwaita-style toggle switch.
///
/// Returns `true` if the value was toggled this frame.
pub fn toggle_anim(
    dl: &mut DrawList,
    inp: &InputState,
    state: &mut ToggleState,
    x: f32,
    y: f32,
    val: &mut bool,
) -> bool {
    let w = 40.0;
    let h = 22.0;
    let r = h * 0.5;
    let thumb_r = r - 3.0;

    state.anim_t = input::smooth(
        state.anim_t,
        if *val { 1.0 } else { 0.0 },
        t::ANIM_NORMAL,
        inp.dt,
    );

    let hov = inp.in_rect(x - 2.0, y - 2.0, w + 4.0, h + 4.0);
    state.hover_t = input::smooth(
        state.hover_t,
        if hov { 1.0 } else { 0.0 },
        t::ANIM_FAST,
        inp.dt,
    );

    let track_bg = t::lerp4(t::TOGGLE_OFF, t::ACCENT, state.anim_t);
    let track_col = t::lerp4(track_bg, t::lerp4(track_bg, [1.0; 4], 0.08), state.hover_t);

    // Track shadow
    dl.aa_rounded_rect(x, y + 1.0, w, h, r, [0.0, 0.0, 0.0, 0.12]);
    // Track fill
    dl.aa_rounded_rect(x, y, w, h, r, track_col);
    // Border
    let border_a = 0.12 - state.anim_t * 0.05;
    dl.aa_rounded_rect(x, y, w, h, r, [1.0, 1.0, 1.0, border_a]);
    dl.aa_rounded_rect(
        x + 1.0, y + 1.0, w - 2.0, h - 2.0, r - 1.0, track_col,
    );

    // Thumb
    let tx = x + r + (w - h) * state.anim_t;
    let ty = y + r;
    dl.aa_circle(tx, ty + 1.0, thumb_r + 0.5, [0.0, 0.0, 0.0, 0.22]);
    dl.aa_circle(tx, ty, thumb_r, t::TEXT);

    let clicked = inp.clicked() && hov;
    if clicked {
        *val = !*val;
    }
    clicked
}

// Keyboard grid

/// Data about the key currently under the cursor.
pub struct HoveredKey {
    pub code: usize,
    pub name: &'static str,
    pub x: f32,
    pub y: f32,
    pub w: f32,
}

/// Aggregated result after drawing the keyboard grid.
pub struct GridResult {
    pub height: f32,
    pub hovered: Option<HoveredKey>,
    pub right_click: Option<(usize, f32, f32)>,
}

/// Persistent state for the keyboard grid widget.
#[derive(Default)]
pub struct GridState {
    pub selected: HashSet<usize>,
    adding: bool,
    dragging: bool,
    hover_t: HashMap<usize, f32>,
    select_t: HashMap<usize, f32>,
}

impl GridState {
    /// Toggle select-all / deselect-all.
    pub fn select_all(&mut self, kl: &[Option<&str>]) {
        if self.selected.is_empty() {
            for (i, e) in kl.iter().enumerate() {
                if e.is_some() {
                    self.selected.insert(i);
                }
            }
        } else {
            self.selected.clear();
        }
    }
}

/// Draw the full keyboard grid and handle selection.
///
/// `interactive` disables click/drag processing (set to `false`
/// when an overlay popup is open to prevent click-through).
///
/// `preview_colors` is an optional per-key-code colour override
/// used for effect animation preview. Entries that are `None`
/// fall back to the per-key colour stored in the key model.
#[allow(clippy::too_many_arguments)]
pub fn keyboard_grid(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    keys: &[Option<Key>],
    state: &mut GridState,
    show_act: bool,
    key_list: &[Option<&str>],
    x0: f32,
    y0: f32,
    avail_w: f32,
    interactive: bool,
    preview_colors: &[Option<[u8; 3]>],
) -> GridResult {
    let cols = 16.0f32;
    let unit = (avail_w / cols).min(t::UNIT_PX);
    let row_h = unit * 0.84;
    let gap = t::KEY_GAP;
    let xoff = (avail_w - cols * unit) * 0.5;

    struct RK {
        code: usize,
        name: &'static str,
        rx: f32,
        ry: f32,
        rw: f32,
        rh: f32,
    }
    let mut rks = Vec::with_capacity(70);
    for (idx, entry) in key_list.iter().enumerate() {
        if let Some(name) = entry {
            if let Some(l) = get_key_layout(name) {
                rks.push(RK {
                    code: idx,
                    name: l.name,
                    rx: x0 + xoff + l.column * unit + gap * 0.5,
                    ry: y0 + l.row as f32 * (row_h + gap),
                    rw: l.width * unit - gap,
                    rh: row_h,
                });
            }
        }
    }

    // Selection input (only when interactive)
    if interactive {
        if inp.just_pressed() {
            for rk in &rks {
                if inp.in_rect(rk.rx, rk.ry, rk.rw, rk.rh) {
                    state.adding = !state.selected.contains(&rk.code);
                    state.dragging = true;
                    break;
                }
            }
        }
        if !inp.mouse_down {
            state.dragging = false;
        }
    }

    let mut result = GridResult {
        height: 5.0 * (row_h + gap),
        hovered: None,
        right_click: None,
    };

    for rk in &rks {
        let hov = interactive && inp.in_rect(rk.rx, rk.ry, rk.rw, rk.rh);
        if interactive && state.dragging && hov {
            if state.adding {
                state.selected.insert(rk.code);
            } else {
                state.selected.remove(&rk.code);
            }
        }
        let is_sel = state.selected.contains(&rk.code);

        let ht = state.hover_t.entry(rk.code).or_insert(0.0);
        *ht = input::smooth(*ht, if hov { 1.0 } else { 0.0 }, t::ANIM_FAST, inp.dt);
        let hover_t = *ht;

        let st = state.select_t.entry(rk.code).or_insert(0.0);
        *st = input::smooth(
            *st,
            if is_sel { 1.0 } else { 0.0 },
            t::ANIM_NORMAL,
            inp.dt,
        );
        let select_t = *st;

        let pressed = interactive && inp.mouse_down && hov;
        let press_dy = if pressed { 1.0 } else { 0.0 };

        // Colour: check preview overlay first, then model colours
        let preview = preview_colors.get(rk.code).and_then(|c| *c);
        let (grad_top, grad_bot) = if let Some([pr, pg, pb]) = preview {
            if pr > 5 || pg > 5 || pb > 5 {
                // Brighter gradient for effect preview
                let base_r = pr as f32 / 255.0 * 0.50 + 0.12;
                let base_g = pg as f32 / 255.0 * 0.50 + 0.12;
                let base_b = pb as f32 / 255.0 * 0.50 + 0.12;
                (
                    [base_r + 0.025, base_g + 0.025, base_b + 0.025, 1.0],
                    [base_r - 0.018, base_g - 0.018, base_b - 0.018, 1.0],
                )
            } else {
                (t::BG_KEY_TOP, t::BG_KEY_BOT)
            }
        } else {
            let has_color = matches!(
                keys.get(rk.code),
                Some(Some(k)) if k.color.r > 0 || k.color.g > 0 || k.color.b > 0
            );
            if has_color {
                let k = keys[rk.code].as_ref().unwrap();
                t::dim_key_gradient(k.color.r, k.color.g, k.color.b)
            } else {
                (t::BG_KEY_TOP, t::BG_KEY_BOT)
            }
        };

        let top = t::lerp4(
            t::lerp4(grad_top, t::BG_KEY_HOVER, hover_t),
            t::BG_KEY_ACTIVE,
            select_t * 0.4,
        );
        let bot = t::lerp4(
            t::lerp4(grad_bot, t::BG_KEY_HOVER, hover_t),
            t::BG_KEY_ACTIVE,
            select_t * 0.4,
        );

        let ky = rk.ry + press_dy;

        // Shadow
        let shadow_a = (0.18 + hover_t * 0.12) * (1.0 - press_dy * 0.5);
        dl.aa_rounded_rect(
            rk.rx, ky + 2.0, rk.rw, rk.rh, t::KEY_R,
            [0.0, 0.0, 0.0, shadow_a],
        );

        // Selection glow
        if select_t > 0.01 {
            let e = 3.0 * select_t;
            dl.aa_rounded_rect(
                rk.rx - e, ky - e, rk.rw + e * 2.0, rk.rh + e * 2.0,
                t::KEY_R + e,
                t::with_alpha(t::ACCENT, 0.15 * select_t),
            );
        }

        // Key body (gradient)
        dl.aa_rounded_rect_gradient_v(rk.rx, ky, rk.rw, rk.rh, t::KEY_R, top, bot);

        // Top highlight
        dl.rect(
            rk.rx + 4.0, ky + 1.0, rk.rw - 8.0, 1.0,
            [1.0, 1.0, 1.0, 0.06 + hover_t * 0.04],
        );

        // Selection border
        if select_t > 0.01 {
            let bw = 2.0;
            dl.aa_rounded_rect(
                rk.rx, ky, rk.rw, rk.rh, t::KEY_R,
                t::with_alpha(t::ACCENT, 0.7 * select_t),
            );
            dl.aa_rounded_rect(
                rk.rx + bw, ky + bw,
                rk.rw - bw * 2.0, rk.rh - bw * 2.0,
                (t::KEY_R - bw).max(0.0),
                t::lerp4(top, bot, 0.5),
            );
        }

        // Key label
        let label_bright = (hover_t * 0.5 + select_t * 0.5).min(1.0);
        fa.draw_centered(
            dl, rk.name, rk.rx + rk.rw * 0.5, ky + rk.rh * 0.5,
            t::FONT_KEY, t::lerp4(t::TEXT_SEC, t::TEXT, label_bright),
        );

        // Actuation overlays
        if show_act {
            if let Some(Some(k)) = keys.get(rk.code) {
                if k.down_actuation > 0.0 {
                    let s = format!("{:.2}", k.down_actuation);
                    let tw = fa.measure(&s, t::FONT_CAPTION).0;
                    fa.draw_text(
                        dl, &s,
                        rk.rx + (rk.rw - tw) * 0.5, ky + rk.rh - 13.0,
                        t::FONT_CAPTION, t::PRESS_COL,
                    );
                }
                if k.up_actuation > 0.0 {
                    let s = format!("{:.2}", k.up_actuation);
                    let tw = fa.measure(&s, t::FONT_CAPTION).0;
                    fa.draw_text(
                        dl, &s,
                        rk.rx + (rk.rw - tw) * 0.5, ky + 2.0,
                        t::FONT_CAPTION, t::RELEASE_COL,
                    );
                }
            }
        }

        // Hover info
        if hover_t > 0.5 {
            result.hovered = Some(HoveredKey {
                code: rk.code,
                name: rk.name,
                x: rk.rx + rk.rw * 0.5,
                y: rk.ry,
                w: rk.rw,
            });
        }
        // Context menu
        if interactive && inp.right_clicked() && hov {
            result.right_click = Some((rk.code, inp.mouse_x, inp.mouse_y));
        }
    }

    result
}

// HSV colour picker

/// Per-picker drag state.
#[derive(Default)]
pub struct HsvPickerState {
    pub hue: f32,
    pub sat: f32,
    pub val: f32,
    pub dragging_ring: bool,
    pub dragging_sv: bool,
}

impl HsvPickerState {
    /// Whether the user is currently dragging any part of the picker.
    pub fn is_dragging(&self) -> bool {
        self.dragging_ring || self.dragging_sv
    }

    /// Synchronise HSV from an external RGB colour (e.g. the key model).
    pub fn sync_from_rgb(&mut self, r: u8, g: u8, b: u8) {
        let (h, s, v) = t::rgb_to_hsv(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
        );
        self.hue = h;
        self.sat = s;
        self.val = v;
    }

    /// Convert the current HSV back to 0-255 RGB.
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        let [r, g, b] = t::hsv_to_rgb(self.hue, self.sat, self.val);
        (
            (r * 255.0).round() as u8,
            (g * 255.0).round() as u8,
            (b * 255.0).round() as u8,
        )
    }
}

/// Draw the HSV colour wheel with saturation/value square.
///
/// Returns `true` when the colour changed.
pub fn hsv_picker(
    dl: &mut DrawList,
    _fa: &FontAtlas,
    inp: &InputState,
    state: &mut HsvPickerState,
    cx: f32,
    cy: f32,
    radius: f32,
) -> bool {
    let r_out = radius;
    let ring_w = 12.0;
    let r_in = r_out - ring_w;
    let hue_seg = 180u32;
    let sv_div = 28u32;

    // Dark backdrop
    dl.aa_circle(cx, cy, r_out + 1.5, [0.0, 0.0, 0.0, 0.18]);

    // SV square
    let half = r_in * 0.70;
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
                [sq_x + i as f32 * step, sq_y + j as f32 * step], c00,
                [sq_x + (i + 1) as f32 * step, sq_y + j as f32 * step], c10,
                [sq_x + (i + 1) as f32 * step, sq_y + (j + 1) as f32 * step], c11,
                [sq_x + i as f32 * step, sq_y + (j + 1) as f32 * step], c01,
            );
        }
    }

    // SV border
    let bdr = [1.0, 1.0, 1.0, 0.06];
    dl.rect(sq_x, sq_y, sq_size, 1.0, bdr);
    dl.rect(sq_x, sq_y + sq_size - 1.0, sq_size, 1.0, bdr);
    dl.rect(sq_x, sq_y, 1.0, sq_size, bdr);
    dl.rect(sq_x + sq_size - 1.0, sq_y, 1.0, sq_size, bdr);

    // Hue ring
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
            [cx + r_in * cos0, cy + r_in * sin0],
            [cx + r_in * cos1, cy + r_in * sin1],
            c0, c1,
        );
    }

    // Soft edges on the ring
    dl.ring(
        cx, cy, r_out, r_out + 1.5, 96,
        [t::BG_RAISED[0], t::BG_RAISED[1], t::BG_RAISED[2], 0.7],
    );
    dl.ring(
        cx, cy, r_in - 1.5, r_in, 96,
        [t::BG_RAISED[0], t::BG_RAISED[1], t::BG_RAISED[2], 0.5],
    );

    // Hue indicator
    let ha = state.hue.to_radians();
    let r_mid = (r_out + r_in) * 0.5;
    let hx = cx + r_mid * ha.cos();
    let hy = cy + r_mid * ha.sin();
    dl.aa_circle(hx, hy + 1.0, 6.0, [0.0, 0.0, 0.0, 0.30]);
    dl.aa_circle(hx, hy, 5.5, [1.0, 1.0, 1.0, 1.0]);
    dl.aa_circle(hx, hy, 3.5, t::hsv_to_rgba(state.hue, 1.0, 1.0));

    // SV indicator
    let sx = sq_x + state.sat * sq_size;
    let sy = sq_y + (1.0 - state.val) * sq_size;
    dl.aa_circle(sx, sy + 1.0, 6.0, [0.0, 0.0, 0.0, 0.30]);
    dl.aa_circle(sx, sy, 5.5, [1.0, 1.0, 1.0, 1.0]);
    dl.aa_circle(sx, sy, 3.5, t::hsv_to_rgba(state.hue, state.sat, state.val));

    // Interaction
    let dx = inp.mouse_x - cx;
    let dy = inp.mouse_y - cy;
    let dist = (dx * dx + dy * dy).sqrt();

    if inp.just_pressed() {
        if dist >= r_in - 2.0 && dist <= r_out + 4.0 {
            state.dragging_ring = true;
        } else if (inp.mouse_x - cx).abs() <= half + 3.0
            && (inp.mouse_y - cy).abs() <= half + 3.0
            && dist < r_in + 2.0
        {
            state.dragging_sv = true;
        }
    }
    if !inp.mouse_down {
        state.dragging_ring = false;
        state.dragging_sv = false;
    }

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

// Colour presets

/// Row of quick-colour buttons (R, G, B, W, Off).
pub fn color_presets(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    keys: &mut [Option<Key>],
    sel: &HashSet<usize>,
    x: f32,
    y: f32,
) -> bool {
    let presets: &[(&str, [u8; 3])] = &[
        ("R", [255, 0, 0]),
        ("G", [0, 255, 0]),
        ("B", [0, 0, 255]),
        ("W", [255, 255, 255]),
        ("Off", [0, 0, 0]),
    ];
    let mut changed = false;
    let mut cx = x;
    for (label, rgb) in presets {
        let bw = fa.measure(label, t::FONT_SMALL).0 + 14.0;
        let col = t::rgb_to_col(rgb[0].max(30), rgb[1].max(30), rgb[2].max(30));
        if button(dl, fa, inp, cx, y, bw, 22.0, label, col, true) {
            let nc = crate::model::key::KeyColor {
                r: rgb[0],
                g: rgb[1],
                b: rgb[2],
            };
            for &c in sel {
                if let Some(Some(k)) = keys.get_mut(c) {
                    k.color = nc;
                }
            }
            changed = true;
        }
        cx += bw + 4.0;
    }
    changed
}

// Tooltip

/// Floating tooltip that appears when hovering a key.
pub fn draw_tooltip(
    dl: &mut DrawList,
    fa: &FontAtlas,
    keys: &[Option<Key>],
    hk: &HoveredKey,
) {
    let k = match keys.get(hk.code) {
        Some(Some(k)) => k,
        _ => return,
    };
    let line1 = hk.name;
    let line2 = format!(
        "Press: {:.2}mm  Rel: {:.2}mm",
        k.down_actuation, k.up_actuation
    );
    let tw = fa
        .measure(line1, t::FONT_SMALL)
        .0
        .max(fa.measure(&line2, t::FONT_SMALL).0)
        + 24.0;
    let th = 44.0;
    let tx = (hk.x - tw * 0.5).max(4.0);
    let ty = hk.y - th - 8.0;

    dl.aa_rounded_rect(tx + 2.0, ty + 2.0, tw, th, 8.0, [0.0, 0.0, 0.0, 0.22]);
    dl.aa_rounded_rect(tx, ty, tw, th, 8.0, t::CARD_BORDER);
    dl.aa_rounded_rect(
        tx + 1.0, ty + 1.0, tw - 2.0, th - 2.0, 7.0, t::BG_SURFACE,
    );
    fa.draw_text(dl, line1, tx + 10.0, ty + 6.0, t::FONT_SMALL, t::TEXT);
    fa.draw_text(
        dl,
        &line2,
        tx + 10.0,
        ty + 23.0,
        t::FONT_SMALL,
        t::TEXT_SEC,
    );
    let sc = t::rgb_to_col(k.color.r, k.color.g, k.color.b);
    dl.aa_rounded_rect(tx + tw - 20.0, ty + 7.0, 12.0, 12.0, 3.0, sc);
}

// Dropdown combobox

/// Per-dropdown persistent state.
#[derive(Default)]
pub struct DropdownState {
    /// Whether the popup list is currently visible.
    pub open: bool,
    /// Vertical scroll offset inside the popup.
    pub scroll_y: f32,
    /// Fade-in animation (0..1).
    pub fade_t: f32,
    /// Screen-space anchor set by `dropdown_button`.
    anchor_x: f32,
    anchor_y: f32,
    anchor_w: f32,
    anchor_h: f32,
}

/// One row in the dropdown popup.
pub struct DropdownItem {
    /// Display text.
    pub label: &'static str,
    /// If true, a thin separator line is drawn above this item.
    pub separator_before: bool,
}

/// Draw the dropdown trigger button.
///
/// Shows `current_label` and a small down-arrow.  Toggles
/// `state.open` when clicked.  Call once per frame; the popup
/// is drawn separately by `dropdown_popup`.
pub fn dropdown_button(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    state: &mut DropdownState,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    current_label: &str,
) {
    state.anchor_x = x;
    state.anchor_y = y;
    state.anchor_w = w;
    state.anchor_h = h;

    let hov = inp.in_rect(x, y, w, h);
    let pressed = hov && inp.mouse_down;

    let bg = if state.open {
        t::lerp4(t::BG_KEY, t::ACCENT, 0.25)
    } else if hov {
        t::BG_KEY_HOVER
    } else {
        t::BG_KEY
    };
    let bg = if pressed {
        t::lerp4(bg, [0.0; 4], 0.12)
    } else {
        bg
    };

    dl.aa_rounded_rect(x, y, w, h, 6.0, bg);

    // 1 px border when open or hovered
    if state.open || hov {
        let ba = if state.open { 0.25 } else { 0.10 };
        dl.aa_rounded_rect(x, y, w, h, 6.0, t::with_alpha(t::ACCENT, ba));
        dl.aa_rounded_rect(
            x + 1.0, y + 1.0, w - 2.0, h - 2.0, 5.0, bg,
        );
    }

    // Label
    let tc = if state.open { t::TEXT } else { t::TEXT_SEC };
    fa.draw_text(
        dl, current_label,
        x + 10.0, y + (h - 12.0) * 0.5,
        t::FONT_SMALL, tc,
    );

    // Down-arrow chevron
    let ax = x + w - 16.0;
    let ay = y + h * 0.5;
    let ac = t::with_alpha(tc, 0.6);
    dl.tri(
        [ax - 3.0, ay - 2.0], ac,
        [ax + 3.0, ay - 2.0], ac,
        [ax, ay + 2.5], ac,
    );

    if hov && inp.clicked() {
        state.open = !state.open;
        state.fade_t = 0.0;
        state.scroll_y = 0.0;
    }
}

/// Draw the dropdown popup overlay.
///
/// Must be called **after** `reset_clip` so the popup paints on
/// top of all other content.  Returns `Some(index)` when the user
/// selects a row.
pub fn dropdown_popup(
    dl: &mut DrawList,
    fa: &FontAtlas,
    inp: &InputState,
    state: &mut DropdownState,
    items: &[DropdownItem],
    current_idx: Option<usize>,
    screen_h: f32,
) -> Option<usize> {
    if !state.open {
        state.fade_t = 0.0;
        return None;
    }

    state.fade_t = input::smooth(state.fade_t, 1.0, t::ANIM_FAST, inp.dt);
    let a = state.fade_t;

    let item_h = 28.0;
    let sep_h = 9.0;
    let pad = 6.0;
    let popup_w = state.anchor_w.max(220.0);

    // Total content height
    let mut total_h: f32 = pad * 2.0;
    for item in items {
        if item.separator_before {
            total_h += sep_h;
        }
        total_h += item_h;
    }

    let max_h = 420.0f32.min(screen_h - 20.0);
    let popup_h = total_h.min(max_h);
    let scrollable = total_h > max_h;

    // Place below the button; flip above if no room
    let below_y = state.anchor_y + state.anchor_h + 4.0;
    let above_y = state.anchor_y - popup_h - 4.0;
    let popup_y = if below_y + popup_h < screen_h - 8.0 {
        below_y
    } else if above_y > 8.0 {
        above_y
    } else {
        below_y.min(screen_h - popup_h - 8.0)
    };
    let popup_x = state.anchor_x;

    // Shadow
    dl.aa_rounded_rect(
        popup_x + 2.0, popup_y + 3.0, popup_w, popup_h, 10.0,
        [0.0, 0.0, 0.0, 0.22 * a],
    );
    // Border
    dl.aa_rounded_rect(
        popup_x, popup_y, popup_w, popup_h, 10.0,
        t::with_alpha(t::CARD_BORDER, a),
    );
    // Background fill
    dl.aa_rounded_rect(
        popup_x + 1.0, popup_y + 1.0,
        popup_w - 2.0, popup_h - 2.0, 9.0,
        t::with_alpha(t::BG_SURFACE, a),
    );

    // Scroll wheel inside popup
    if scrollable && inp.in_rect(popup_x, popup_y, popup_w, popup_h) {
        state.scroll_y -= inp.scroll_delta * 30.0;
        state.scroll_y = state.scroll_y.clamp(0.0, (total_h - popup_h).max(0.0));
    }

    // Clip content to the popup interior
    dl.push_clip(popup_x, popup_y + pad, popup_w, popup_h - pad * 2.0);

    let mut iy = popup_y + pad - state.scroll_y;
    let mut selected = None;

    for (idx, item) in items.iter().enumerate() {
        if item.separator_before {
            let sy = iy + sep_h * 0.5;
            if sy > popup_y && sy < popup_y + popup_h {
                dl.rect(
                    popup_x + 12.0, sy, popup_w - 24.0, 1.0,
                    t::with_alpha(t::SEPARATOR, a),
                );
            }
            iy += sep_h;
        }

        let row_y = iy;
        let in_popup = row_y + item_h > popup_y && row_y < popup_y + popup_h;
        let hov = in_popup
            && inp.in_rect(popup_x + 4.0, row_y, popup_w - 8.0, item_h)
            && inp.in_rect(popup_x, popup_y, popup_w, popup_h);
        let is_cur = current_idx == Some(idx);

        if in_popup {
            // Highlight current
            if is_cur {
                dl.aa_rounded_rect(
                    popup_x + 4.0, row_y + 1.0,
                    popup_w - 8.0, item_h - 2.0, 6.0,
                    t::with_alpha(t::ACCENT_DIM, a),
                );
            }
            // Hover highlight
            if hov && !is_cur {
                dl.aa_rounded_rect(
                    popup_x + 4.0, row_y + 1.0,
                    popup_w - 8.0, item_h - 2.0, 6.0,
                    [1.0, 1.0, 1.0, 0.05 * a],
                );
            }

            let tc = if is_cur || hov {
                t::TEXT
            } else {
                t::TEXT_SEC
            };
            fa.draw_text(
                dl, item.label,
                popup_x + 14.0,
                row_y + (item_h - 12.0) * 0.5,
                t::FONT_SMALL,
                t::with_alpha(tc, a),
            );

            // Checkmark for current selection
            if is_cur {
                fa.draw_text(
                    dl, "\u{2713}",
                    popup_x + popup_w - 24.0,
                    row_y + (item_h - 12.0) * 0.5,
                    t::FONT_SMALL,
                    t::with_alpha(t::TEXT, a),
                );
            }
        }

        if hov && inp.clicked() {
            selected = Some(idx);
        }
        iy += item_h;
    }

    dl.pop_clip();

    // Scrollbar
    if scrollable {
        let track = popup_h - pad * 2.0;
        let bar_h = (track * popup_h / total_h).max(20.0);
        let bar_y = popup_y + pad + state.scroll_y / total_h * track;
        dl.aa_rounded_rect(
            popup_x + popup_w - 7.0, bar_y, 3.0, bar_h, 1.5,
            [1.0, 1.0, 1.0, 0.10 * a],
        );
    }

    // Close when clicking outside (but not on the trigger button)
    let on_btn = inp.in_rect(
        state.anchor_x, state.anchor_y,
        state.anchor_w, state.anchor_h,
    );
    if (inp.clicked() || inp.right_clicked())
        && !inp.in_rect(popup_x, popup_y, popup_w, popup_h)
        && !on_btn
    {
        state.open = false;
    }

    if selected.is_some() {
        state.open = false;
    }

    selected
}