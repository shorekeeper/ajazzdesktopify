use std::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    pub pos:   [f32; 2],
    pub uv:    [f32; 2],
    pub color: [f32; 4],
}

pub struct DrawCmd {
    pub index_count:  u32,
    pub index_offset: u32,
    pub clip: [i32; 4],
}

pub struct DrawList {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
    pub commands: Vec<DrawCmd>,
    clip: [i32; 4],
}

const WHITE_UV: [f32; 2] = [0.5 / 512.0, 0.5 / 512.0];

/// Segments per corner of a rounded rect. 12 gives sub-pixel
/// chord error even at 16 px radius — visually indistinguishable
/// from a true arc.
const CORNER_SEG: u32 = 12;

/// Segments for a full circle.  64 is smooth even at r = 80+.
const CIRCLE_SEG: u32 = 64;

impl DrawList {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            vertices: Vec::with_capacity(48_000),
            indices:  Vec::with_capacity(96_000),
            commands: Vec::new(),
            clip: [0, 0, w as i32, h as i32],
        }
    }

    pub fn clear(&mut self, w: u32, h: u32) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.clip = [0, 0, w as i32, h as i32];
    }

    pub fn set_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.clip = [x as i32, y as i32, w as i32, h as i32];
    }

    pub fn reset_clip(&mut self, sw: u32, sh: u32) {
        self.clip = [0, 0, sw as i32, sh as i32];
    }

    fn emit(&mut self, idx_count: u32, idx_offset: u32) {
        if let Some(last) = self.commands.last_mut() {
            if last.clip == self.clip && last.index_offset + last.index_count == idx_offset {
                last.index_count += idx_count;
                return;
            }
        }
        self.commands.push(DrawCmd { index_count: idx_count, index_offset: idx_offset, clip: self.clip });
    }

    // ─── Solid primitives ──────────────────────────────────────

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x, y],         uv: WHITE_UV, color },
            Vertex { pos: [x + w, y],     uv: WHITE_UV, color },
            Vertex { pos: [x + w, y + h], uv: WHITE_UV, color },
            Vertex { pos: [x, y + h],     uv: WHITE_UV, color },
        ]);
        let io = self.indices.len() as u32;
        self.indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
        self.emit(6, io);
    }

    pub fn rounded_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, color: [f32; 4]) {
        if r < 0.5 || w < r * 2.0 || h < r * 2.0 {
            return self.rect(x, y, w, h, color);
        }
        let b = self.vertices.len() as u32;
        let seg = CORNER_SEG;
        self.vertices.push(Vertex { pos: [x + w * 0.5, y + h * 0.5], uv: WHITE_UV, color });
        let corners = [
            (x + r,     y + r,     PI,       PI * 1.5),
            (x + w - r, y + r,     PI * 1.5, PI * 2.0),
            (x + w - r, y + h - r, 0.0,      PI * 0.5),
            (x + r,     y + h - r, PI * 0.5, PI),
        ];
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                self.vertices.push(Vertex { pos: [cx + r * a.cos(), cy + r * a.sin()], uv: WHITE_UV, color });
            }
        }
        let n = 4 * (seg + 1);
        let io = self.indices.len() as u32;
        for i in 0..n {
            self.indices.extend_from_slice(&[b, b + 1 + i, b + 1 + (i + 1) % n]);
        }
        self.emit(n * 3, io);
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: [f32; 4]) {
        let b = self.vertices.len() as u32;
        let seg = CIRCLE_SEG;
        self.vertices.push(Vertex { pos: [cx, cy], uv: WHITE_UV, color });
        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            self.vertices.push(Vertex { pos: [cx + r * a.cos(), cy + r * a.sin()], uv: WHITE_UV, color });
        }
        let io = self.indices.len() as u32;
        for i in 0..seg {
            self.indices.extend_from_slice(&[b, b + 1 + i, b + 1 + (i + 1) % seg]);
        }
        self.emit(seg * 3, io);
    }

    /// Ring (annulus) — useful for the HSV hue wheel soft edges.
    pub fn ring(&mut self, cx: f32, cy: f32, r_inner: f32, r_outer: f32, seg: u32, color: [f32; 4]) {
        let b = self.vertices.len() as u32;
        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            let (s, c_a) = (a.sin(), a.cos());
            self.vertices.push(Vertex { pos: [cx + r_inner * c_a,  cy + r_inner * s],  uv: WHITE_UV, color });
            self.vertices.push(Vertex { pos: [cx + r_outer * c_a, cy + r_outer * s], uv: WHITE_UV, color });
        }
        let io = self.indices.len() as u32;
        let mut count = 0u32;
        for i in 0..seg {
            let i0 = i * 2;
            let i1 = ((i + 1) % seg) * 2;
            self.indices.extend_from_slice(&[b+i0, b+i0+1, b+i1+1, b+i0, b+i1+1, b+i1]);
            count += 6;
        }
        self.emit(count, io);
    }

    // ─── Per-vertex colour primitives (gradients, HSV) ─────────

    pub fn tri(
        &mut self,
        p0: [f32; 2], c0: [f32; 4],
        p1: [f32; 2], c1: [f32; 4],
        p2: [f32; 2], c2: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: p0, uv: WHITE_UV, color: c0 },
            Vertex { pos: p1, uv: WHITE_UV, color: c1 },
            Vertex { pos: p2, uv: WHITE_UV, color: c2 },
        ]);
        let io = self.indices.len() as u32;
        self.indices.extend_from_slice(&[b, b+1, b+2]);
        self.emit(3, io);
    }

    pub fn quad_colors(
        &mut self,
        p0: [f32; 2], c0: [f32; 4],
        p1: [f32; 2], c1: [f32; 4],
        p2: [f32; 2], c2: [f32; 4],
        p3: [f32; 2], c3: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: p0, uv: WHITE_UV, color: c0 },
            Vertex { pos: p1, uv: WHITE_UV, color: c1 },
            Vertex { pos: p2, uv: WHITE_UV, color: c2 },
            Vertex { pos: p3, uv: WHITE_UV, color: c3 },
        ]);
        let io = self.indices.len() as u32;
        self.indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
        self.emit(6, io);
    }

    /// Hue-ring segment: two triangles forming a trapezoid, each
    /// corner carrying its own colour for smooth interpolation.
    pub fn ring_segment(
        &mut self,
        o0: [f32; 2], o1: [f32; 2],
        i0: [f32; 2], i1: [f32; 2],
        c0: [f32; 4], c1: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: o0, uv: WHITE_UV, color: c0 },
            Vertex { pos: o1, uv: WHITE_UV, color: c1 },
            Vertex { pos: i1, uv: WHITE_UV, color: c1 },
            Vertex { pos: i0, uv: WHITE_UV, color: c0 },
        ]);
        let io = self.indices.len() as u32;
        self.indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
        self.emit(6, io);
    }

    /// Horizontal gradient (left → right).
    pub fn gradient_rect(&mut self, x: f32, y: f32, w: f32, h: f32, left: [f32; 4], right: [f32; 4]) {
        self.quad_colors([x,y], left, [x+w,y], right, [x+w,y+h], right, [x,y+h], left);
    }

    // ─── Text glyph ────────────────────────────────────────────

    pub fn glyph(
        &mut self,
        x: f32, y: f32, w: f32, h: f32,
        u0: f32, v0: f32, u1: f32, v1: f32,
        color: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x, y],         uv: [u0, v0], color },
            Vertex { pos: [x + w, y],     uv: [u1, v0], color },
            Vertex { pos: [x + w, y + h], uv: [u1, v1], color },
            Vertex { pos: [x, y + h],     uv: [u0, v1], color },
        ]);
        let io = self.indices.len() as u32;
        self.indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
        self.emit(6, io);
    }
}