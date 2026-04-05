//! GTK Adwaita Dark inspired palette and layout constants.

// ─── Backgrounds ───────────────────────────────────────────
pub const BG_BASE:       [f32; 4] = [0.110, 0.110, 0.110, 1.0];
pub const BG_SURFACE:    [f32; 4] = [0.165, 0.165, 0.165, 1.0];
pub const BG_RAISED:     [f32; 4] = [0.212, 0.212, 0.212, 1.0];
pub const BG_KEY:        [f32; 4] = [0.188, 0.188, 0.188, 1.0];
pub const BG_KEY_HOVER:  [f32; 4] = [0.232, 0.232, 0.232, 1.0];
pub const BG_KEY_ACTIVE: [f32; 4] = [0.260, 0.260, 0.260, 1.0];

// ─── Borders & decorations ─────────────────────────────────
pub const BORDER:        [f32; 4] = [1.0, 1.0, 1.0, 0.08];
pub const CARD_BORDER:   [f32; 4] = [1.0, 1.0, 1.0, 0.05];
pub const SEPARATOR:     [f32; 4] = [1.0, 1.0, 1.0, 0.06];

// ─── Toggle track ──────────────────────────────────────────
/// OFF track — much darker than BG_RAISED so it stands out
/// inside mini-cards.
pub const TOGGLE_OFF:     [f32; 4] = [0.125, 0.125, 0.125, 1.0];
pub const TOGGLE_BORDER:  [f32; 4] = [1.0, 1.0, 1.0, 0.12];

// ─── Slider track ──────────────────────────────────────────
pub const SLIDER_TRACK:   [f32; 4] = [0.145, 0.145, 0.145, 1.0];
pub const SLIDER_TRACK_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 0.05];

// ─── Accent ────────────────────────────────────────────────
pub const ACCENT:        [f32; 4] = [0.208, 0.518, 0.894, 1.0];
pub const ACCENT_DIM:    [f32; 4] = [0.208, 0.518, 0.894, 0.15];

// ─── Semantic ──────────────────────────────────────────────
pub const GREEN:  [f32; 4] = [0.200, 0.820, 0.600, 1.0];
pub const AMBER:  [f32; 4] = [0.980, 0.753, 0.141, 1.0];
pub const RED:    [f32; 4] = [0.970, 0.440, 0.440, 1.0];

// ─── Text ──────────────────────────────────────────────────
pub const TEXT:     [f32; 4] = [0.960, 0.960, 0.960, 1.0];
pub const TEXT_SEC: [f32; 4] = [0.620, 0.620, 0.620, 1.0];
pub const TEXT_DIM: [f32; 4] = [0.420, 0.420, 0.420, 1.0];

pub const PRESS_COL:   [f32; 4] = [0.580, 0.773, 0.992, 0.75];
pub const RELEASE_COL: [f32; 4] = [0.992, 0.898, 0.541, 0.75];

// ─── Layout ────────────────────────────────────────────────
pub const TOP_BAR_H:   f32 = 48.0;
pub const BOT_BAR_H:   f32 = 52.0;
pub const KEY_GAP:     f32 = 3.0;
pub const KEY_R:       f32 = 6.0;
pub const CARD_R:      f32 = 12.0;
pub const MINI_CARD_R: f32 = 8.0;
pub const UNIT_PX:     f32 = 56.0;
pub const CARD_PAD:    f32 = 16.0;

// ─── Typography ────────────────────────────────────────────
pub const FONT_TINY:   f32 = 8.0;
pub const FONT_SMALL:  f32 = 12.0;
pub const FONT_NORMAL: f32 = 14.0;
pub const FONT_KEY:    f32 = 12.0;
pub const FONT_HEADER: f32 = 15.0;
pub const FONT_TITLE:  f32 = 24.0;

// ─── Animation ─────────────────────────────────────────────
pub const ANIM_FAST:   f32 = 14.0;
pub const ANIM_NORMAL: f32 = 8.0;
pub const ANIM_SLOW:   f32 = 5.0;

// ─── Helpers ───────────────────────────────────────────────

#[inline]
pub fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let s = 1.0 - t;
    [a[0]*s+b[0]*t, a[1]*s+b[1]*t, a[2]*s+b[2]*t, a[3]*s+b[3]*t]
}

#[inline]
pub fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] { [c[0], c[1], c[2], a] }

pub fn rgb_to_col(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub fn dim_key_color(r: u8, g: u8, b: u8) -> [f32; 4] {
    [
        (r as f32 * 0.35 / 255.0) + 0.10,
        (g as f32 * 0.35 / 255.0) + 0.10,
        (b as f32 * 0.35 / 255.0) + 0.10,
        1.0,
    ]
}

// ─── HSV ↔ RGB ─────────────────────────────────────────────

pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    let m = v - c;
    [r1 + m, g1 + m, b1 + m]
}

pub fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let [r, g, b] = hsv_to_rgb(h, s, v);
    [r, g, b, 1.0]
}

pub fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max < 1e-6 { 0.0 } else { d / max };
    let h = if d < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / d) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (if h < 0.0 { h + 360.0 } else { h }, s, max)
}