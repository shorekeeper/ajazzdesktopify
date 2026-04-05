use std::collections::HashMap;
use crate::ui::draw::DrawList;

pub const ATLAS_W: u32 = 512;
pub const ATLAS_H: u32 = 512;

struct GlyphEntry { ax: u32, ay: u32, w: u32, h: u32, off_x: f32, off_y: f32, advance: f32 }

pub struct FontAtlas {
    pub pixels: Vec<u8>,
    glyphs: HashMap<(char, u16), GlyphEntry>,
    font: fontdue::Font,
    cursor_x: u32,
    cursor_y: u32,
    row_h: u32,
}

fn size_key(s: f32) -> u16 { (s * 10.0) as u16 }

impl FontAtlas {
    pub fn new() -> Self {
        let paths = [
            "C:\\Windows\\Fonts\\bahnschrift.ttf",
            "C:\\Windows\\Fonts\\segoeui.ttf",
            "C:\\Windows\\Fonts\\arial.ttf",
            "C:\\Windows\\Fonts\\tahoma.ttf",
        ];
        let data = paths.iter().find_map(|p| std::fs::read(p).ok()).expect("No system font found");
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default()).expect("Failed to parse font");

        let mut atlas = Self {
            pixels: vec![0u8; (ATLAS_W * ATLAS_H) as usize],
            glyphs: HashMap::new(),
            font,
            cursor_x: 4, cursor_y: 0, row_h: 0,
        };

        // White pixel block for solid fills.
        for y in 0..3u32 { for x in 0..3u32 { atlas.pixels[(y * ATLAS_W + x) as usize] = 255; } }

        let sizes = [8.0f32, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 22.0, 24.0];
        for &sz in &sizes {
            for c in 32u8..=126 { atlas.rasterize(c as char, sz); }
            // Unicode icons for buttons.
            for &c in &['\u{2713}', '\u{25C6}', '\u{25B8}', '\u{2022}'] {
                atlas.rasterize(c, sz);
            }
        }
        atlas
    }

    fn rasterize(&mut self, ch: char, size: f32) {
        let key = (ch, size_key(size));
        if self.glyphs.contains_key(&key) { return; }
        let (metrics, bitmap) = self.font.rasterize(ch, size);
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        if w == 0 || h == 0 {
            self.glyphs.insert(key, GlyphEntry { ax: 0, ay: 0, w: 0, h: 0, off_x: metrics.xmin as f32, off_y: 0.0, advance: metrics.advance_width });
            return;
        }
        if self.cursor_x + w + 1 > ATLAS_W { self.cursor_y += self.row_h + 1; self.cursor_x = 0; self.row_h = 0; }
        if self.cursor_y + h + 1 > ATLAS_H { log::warn!("Font atlas full for '{ch}' at {size}"); return; }
        let (ax, ay) = (self.cursor_x, self.cursor_y);
        for row in 0..h {
            let src = (row * w) as usize;
            let dst = ((ay + row) * ATLAS_W + ax) as usize;
            self.pixels[dst..dst + w as usize].copy_from_slice(&bitmap[src..src + w as usize]);
        }
        self.cursor_x += w + 1;
        self.row_h = self.row_h.max(h);
        let ascent = self.font.horizontal_line_metrics(size).map(|l| l.ascent).unwrap_or(size * 0.8);
        let off_y = ascent - metrics.ymin as f32 - h as f32;
        self.glyphs.insert(key, GlyphEntry { ax, ay, w, h, off_x: metrics.xmin as f32, off_y, advance: metrics.advance_width });
    }

    pub fn measure(&self, text: &str, size: f32) -> (f32, f32) {
        let ks = size_key(size);
        let mut w = 0.0f32;
        let h = self.font.horizontal_line_metrics(size).map(|l| l.ascent - l.descent).unwrap_or(size);
        for ch in text.chars() { if let Some(g) = self.glyphs.get(&(ch, ks)) { w += g.advance; } }
        (w, h)
    }

    pub fn draw_text(&self, dl: &mut DrawList, text: &str, x: f32, y: f32, size: f32, color: [f32; 4]) {
        let ks = size_key(size);
        let (aw, ah) = (ATLAS_W as f32, ATLAS_H as f32);
        let mut cx = x;
        for ch in text.chars() {
            if let Some(g) = self.glyphs.get(&(ch, ks)) {
                if g.w > 0 && g.h > 0 {
                    let gx = (cx + g.off_x).round();
                    let gy = (y + g.off_y).round();
                    dl.glyph(gx, gy, g.w as f32, g.h as f32,
                        g.ax as f32 / aw, g.ay as f32 / ah,
                        (g.ax + g.w) as f32 / aw, (g.ay + g.h) as f32 / ah, color);
                }
                cx += g.advance;
            }
        }
    }

    pub fn draw_centered(&self, dl: &mut DrawList, text: &str, cx: f32, cy: f32, size: f32, color: [f32; 4]) {
        let (tw, th) = self.measure(text, size);
        self.draw_text(dl, text, (cx - tw * 0.5).round(), (cy - th * 0.5).round(), size, color);
    }
}