/// GTK Adwaita Dark inspired colour palette, layout metrics, and
/// typography constants.
///
/// All colours are linear RGBA with pre-multiplied alpha = 1.0
/// unless stated otherwise.  Layout values are in screen pixels.
///
/// The palette was calibrated against the GNOME 44 Adwaita Dark
/// stylesheet, with minor adjustments for legibility on LCD panels
/// that do not use sub-pixel rendering.

// Backgrounds

/// Application canvas behind every card.
pub const BG_BASE: [f32; 4] = [0.141, 0.141, 0.141, 1.0];

/// Card and panel surface.
pub const BG_SURFACE: [f32; 4] = [0.188, 0.188, 0.188, 1.0];

/// Elevated surface inside cards (mini-cards, list rows).
pub const BG_RAISED: [f32; 4] = [0.220, 0.220, 0.220, 1.0];

/// Key cap default background.
pub const BG_KEY: [f32; 4] = [0.227, 0.227, 0.227, 1.0];

/// Key cap background when the cursor hovers over it.
pub const BG_KEY_HOVER: [f32; 4] = [0.271, 0.271, 0.271, 1.0];

/// Key cap background when the key is in the selection set.
pub const BG_KEY_ACTIVE: [f32; 4] = [0.300, 0.300, 0.300, 1.0];

/// Top gradient for key caps (slightly lighter than base).
pub const BG_KEY_TOP: [f32; 4] = [0.250, 0.250, 0.250, 1.0];

/// Bottom gradient for key caps (slightly darker than base).
pub const BG_KEY_BOT: [f32; 4] = [0.208, 0.208, 0.208, 1.0];

// Borders and decorations

/// Thin lines between major layout areas.
pub const BORDER: [f32; 4] = [1.0, 1.0, 1.0, 0.08];

/// Card outline (very subtle).
pub const CARD_BORDER: [f32; 4] = [1.0, 1.0, 1.0, 0.06];

/// Vertical or horizontal separators inside cards.
pub const SEPARATOR: [f32; 4] = [1.0, 1.0, 1.0, 0.06];

// Toggle track

/// OFF state track colour (must be visible inside mini-cards).
pub const TOGGLE_OFF: [f32; 4] = [0.145, 0.145, 0.145, 1.0];

// Slider track

/// Recessed track behind the slider thumb.
pub const SLIDER_TRACK: [f32; 4] = [0.145, 0.145, 0.145, 1.0];

// Accent

/// Primary accent (Adwaita Blue #3584e4).
pub const ACCENT: [f32; 4] = [0.208, 0.518, 0.894, 1.0];

/// Translucent accent for backgrounds behind active tabs, etc.
pub const ACCENT_DIM: [f32; 4] = [0.208, 0.518, 0.894, 0.15];

// Semantic colours

/// Success / connected.
pub const GREEN: [f32; 4] = [0.200, 0.820, 0.478, 1.0];

/// Warning / unsaved.
pub const AMBER: [f32; 4] = [0.965, 0.827, 0.176, 1.0];

/// Error / destructive.
pub const RED: [f32; 4] = [0.878, 0.106, 0.141, 1.0];

// Text

/// Primary text on dark backgrounds.
pub const TEXT: [f32; 4] = [0.960, 0.960, 0.960, 1.0];

/// Secondary text (labels, hints).
pub const TEXT_SEC: [f32; 4] = [0.620, 0.620, 0.620, 1.0];

/// Disabled or barely-visible text.
pub const TEXT_DIM: [f32; 4] = [0.420, 0.420, 0.420, 1.0];

/// Colour used for "Press" actuation labels and slider fills.
pub const PRESS_COL: [f32; 4] = [0.580, 0.773, 0.992, 0.85];

/// Colour used for "Release" actuation labels and slider fills.
pub const RELEASE_COL: [f32; 4] = [0.992, 0.898, 0.541, 0.85];

// Layout metrics (pixels)

/// Height of the top bar that holds the title and layer tabs.
pub const TOP_BAR_H: f32 = 48.0;

/// Height of the bottom bar that holds Apply and action buttons.
pub const BOT_BAR_H: f32 = 52.0;

/// Gap between adjacent key caps.
pub const KEY_GAP: f32 = 3.0;

/// Corner radius of key caps.
pub const KEY_R: f32 = 6.0;

/// Corner radius of cards.
pub const CARD_R: f32 = 12.0;

/// Corner radius of mini-cards inside the options panel.
pub const MINI_CARD_R: f32 = 8.0;

/// Width of one standard key unit (1U) in pixels.
pub const UNIT_PX: f32 = 56.0;

/// Inner padding of cards.
pub const CARD_PAD: f32 = 16.0;

// Typography (font sizes in pixels)

/// Very small captions (actuation overlays on keys).
pub const FONT_CAPTION: f32 = 10.0;

/// Small labels (slider labels, tab names, badges).
pub const FONT_SMALL: f32 = 12.0;

/// Body-sized text (slider values, key names, short descriptions).
pub const FONT_BODY: f32 = 13.0;

/// General-purpose text (buttons, list items).
pub const FONT_NORMAL: f32 = 14.0;

/// Key cap label.
pub const FONT_KEY: f32 = 12.0;

/// Section headers and top-bar title.
pub const FONT_HEADER: f32 = 16.0;

/// Large display text (connection screen title).
pub const FONT_TITLE: f32 = 24.0;

// Animation speeds (higher = faster convergence)

/// Fast hover and press feedback.
pub const ANIM_FAST: f32 = 14.0;

/// Standard widget transitions.
pub const ANIM_NORMAL: f32 = 8.0;

/// Slow, cinematic fades.
pub const ANIM_SLOW: f32 = 5.0;

// Helper functions

/// Linearly interpolate two RGBA colours.
#[inline]
pub fn lerp4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let s = 1.0 - t;
    [
        a[0] * s + b[0] * t,
        a[1] * s + b[1] * t,
        a[2] * s + b[2] * t,
        a[3] * s + b[3] * t,
    ]
}

/// Return `c` with a different alpha channel.
#[inline]
pub fn with_alpha(c: [f32; 4], a: f32) -> [f32; 4] {
    [c[0], c[1], c[2], a]
}

/// Convert 0-255 RGB to a linear RGBA colour with alpha 1.
pub fn rgb_to_col(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Build a dimmed key-cap tint from a per-key RGB colour.
///
/// The result is dark enough to keep white text readable but
/// bright enough that the tint is clearly visible.
pub fn dim_key_color(r: u8, g: u8, b: u8) -> [f32; 4] {
    [
        (r as f32 * 0.30 / 255.0) + 0.14,
        (g as f32 * 0.30 / 255.0) + 0.14,
        (b as f32 * 0.30 / 255.0) + 0.14,
        1.0,
    ]
}

/// Build top/bottom gradient pair from a dimmed key colour.
pub fn dim_key_gradient(r: u8, g: u8, b: u8) -> ([f32; 4], [f32; 4]) {
    let base = dim_key_color(r, g, b);
    let top = [base[0] + 0.025, base[1] + 0.025, base[2] + 0.025, 1.0];
    let bot = [base[0] - 0.018, base[1] - 0.018, base[2] - 0.018, 1.0];
    (top, bot)
}

// HSV / RGB conversion

/// Convert HSV (h in degrees, s and v in 0..1) to linear RGB.
pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let c = v * s;
    let hp = h / 60.0;
    let x = c * (1.0 - ((hp % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r1 + m, g1 + m, b1 + m]
}

/// HSV to RGBA with alpha 1.
pub fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let [r, g, b] = hsv_to_rgb(h, s, v);
    [r, g, b, 1.0]
}

/// RGB (each 0..1) to HSV.
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