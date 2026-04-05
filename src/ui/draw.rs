/// 2D immediate-mode rendering primitives for the UI layer.
///
/// This module is the core rendering library of the project.
/// It accumulates geometry into vertex and index buffers during
/// a single frame, batches draw calls by scissor region, and
/// provides both solid and anti-aliased shape primitives.
///
/// Anti-aliased shapes emit an extra ring of gradually transparent
/// vertices along the outline, producing smooth edges without
/// requiring multi-sample anti-aliasing at the GPU level.
///
/// # Quick start
///
/// ```rust,ignore
/// let mut dl = DrawList::new(width, height);
/// dl.clear(width, height);
/// dl.aa_rounded_rect(10.0, 20.0, 200.0, 40.0, 8.0, SOME_COLOR);
/// dl.aa_circle(100.0, 100.0, 20.0, SOME_COLOR);
/// // hand dl.vertices / dl.indices / dl.commands to the Vulkan backend
/// ```
///
/// # Coordinate system
///
/// All positions are in screen pixels with origin at the top-left
/// corner. The vertex shader converts them to NDC using a push
/// constant that carries the current window size.

use std::f32::consts::PI;

/// Single GPU vertex matching the pipeline layout.
///
/// ```text
/// location 0  pos    vec2   offset  0
/// location 1  uv     vec2   offset  8
/// location 2  color  vec4   offset 16
/// total stride: 32 bytes
/// ```
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Vertex {
    /// Screen-space position in pixels.
    pub pos: [f32; 2],
    /// Texture coordinate into the font atlas.
    /// Solid primitives use the white pixel UV.
    pub uv: [f32; 2],
    /// Linear RGBA colour.
    pub color: [f32; 4],
}

/// A batched draw command covering a contiguous range of indices
/// that share the same scissor rectangle.
pub struct DrawCmd {
    /// How many indices this command draws.
    pub index_count: u32,
    /// Byte offset into the index buffer where this command starts.
    pub index_offset: u32,
    /// Scissor clip rectangle as `[x, y, width, height]` in pixels.
    pub clip: [i32; 4],
}

/// UV that points at the tiny solid white block rasterised into
/// the top-left corner of the font atlas (3x3 pixels).
const WHITE_UV: [f32; 2] = [1.0 / 512.0, 1.0 / 512.0];

/// Vertices per quarter-arc for anti-aliased rounded shapes.
/// 16 gives sub-pixel chord error at radii up to about 40 px.
const AA_CORNER_SEG: u32 = 8;

/// Vertices per quarter-arc for solid (non-AA) rounded shapes.
const CORNER_SEG: u32 = 10;

/// Vertices for a full circle.
const CIRCLE_SEG: u32 = 48;

/// Width of the translucent fringe used for anti-aliased edges.
const AA_FRINGE: f32 = 1.0;

/// Linearly interpolate two RGBA colours.
#[inline]
fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Return `color` with the alpha channel replaced.
#[inline]
fn with_zero_alpha(c: [f32; 4]) -> [f32; 4] {
    [c[0], c[1], c[2], 0.0]
}

/// The main draw list that accumulates a frame of geometry.
///
/// Vertex and index data are written into flat `Vec`s.  Each
/// group of primitives that shares the same scissor rectangle
/// is collapsed into one `DrawCmd`.
///
/// After the frame is built, hand `vertices`, `indices`, and
/// `commands` to the Vulkan renderer for GPU submission.
pub struct DrawList {
    /// All vertices emitted this frame.
    pub vertices: Vec<Vertex>,
    /// All triangle indices emitted this frame.
    pub indices: Vec<u32>,
    /// Batched draw commands.
    pub commands: Vec<DrawCmd>,
    clip: [i32; 4],
    clip_stack: Vec<[i32; 4]>,
}

impl DrawList {
    /// Create a draw list pre-allocated for a window of the given size.
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            vertices: Vec::with_capacity(80_000),
            indices: Vec::with_capacity(160_000),
            commands: Vec::new(),
            clip: [0, 0, w as i32, h as i32],
            clip_stack: Vec::with_capacity(8),
        }
    }

    /// Reset every buffer so the draw list is ready for a new frame.
    pub fn clear(&mut self, w: u32, h: u32) {
        self.vertices.clear();
        self.indices.clear();
        self.commands.clear();
        self.clip = [0, 0, w as i32, h as i32];
        self.clip_stack.clear();
    }

    /// Overwrite the current scissor rectangle.
    pub fn set_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.clip = [x as i32, y as i32, w as i32, h as i32];
    }

    /// Reset the scissor rectangle to cover the whole window.
    pub fn reset_clip(&mut self, sw: u32, sh: u32) {
        self.clip = [0, 0, sw as i32, sh as i32];
    }

    /// Push the current scissor onto a stack and set a new one.
    pub fn push_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.clip_stack.push(self.clip);
        self.clip = [x as i32, y as i32, w as i32, h as i32];
    }

    /// Pop the most recently pushed scissor rectangle.
    pub fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip = prev;
        }
    }

    /// Record or extend a draw command for the indices just emitted.
    fn emit(&mut self, idx_count: u32, idx_offset: u32) {
        if let Some(last) = self.commands.last_mut() {
            if last.clip == self.clip
                && last.index_offset + last.index_count == idx_offset
            {
                last.index_count += idx_count;
                return;
            }
        }
        self.commands.push(DrawCmd {
            index_count: idx_count,
            index_offset: idx_offset,
            clip: self.clip,
        });
    }

    // Solid primitives (no anti-aliasing fringe)

    /// Axis-aligned solid rectangle.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x, y], uv: WHITE_UV, color },
            Vertex { pos: [x + w, y], uv: WHITE_UV, color },
            Vertex { pos: [x + w, y + h], uv: WHITE_UV, color },
            Vertex { pos: [x, y + h], uv: WHITE_UV, color },
        ]);
        let io = self.indices.len() as u32;
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        self.emit(6, io);
    }

    /// Solid rounded rectangle without anti-aliasing.
    ///
    /// Falls back to a plain `rect` when the radius is negligible.
    pub fn rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: [f32; 4],
    ) {
        let r = r.min(w * 0.5).min(h * 0.5);
        if r < 0.5 || w < 1.0 || h < 1.0 {
            return self.rect(x, y, w, h, color);
        }
        self.fill_rounded_rect_core(x, y, w, h, r, CORNER_SEG, color);
    }

    /// Internal fan-fill for a rounded rectangle with uniform colour.
    fn fill_rounded_rect_core(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        seg: u32,
        color: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            pos: [x + w * 0.5, y + h * 0.5],
            uv: WHITE_UV,
            color,
        });
        let corners = [
            (x + r, y + r, PI, PI * 1.5),
            (x + w - r, y + r, PI * 1.5, PI * 2.0),
            (x + w - r, y + h - r, 0.0, PI * 0.5),
            (x + r, y + h - r, PI * 0.5, PI),
        ];
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                self.vertices.push(Vertex {
                    pos: [cx + r * a.cos(), cy + r * a.sin()],
                    uv: WHITE_UV,
                    color,
                });
            }
        }
        let n = 4 * (seg + 1);
        let io = self.indices.len() as u32;
        for i in 0..n {
            self.indices
                .extend_from_slice(&[b, b + 1 + i, b + 1 + (i + 1) % n]);
        }
        self.emit(n * 3, io);
    }

    /// Solid filled circle.
    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, color: [f32; 4]) {
        let seg = CIRCLE_SEG;
        let b = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            pos: [cx, cy],
            uv: WHITE_UV,
            color,
        });
        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            self.vertices.push(Vertex {
                pos: [cx + r * a.cos(), cy + r * a.sin()],
                uv: WHITE_UV,
                color,
            });
        }
        let io = self.indices.len() as u32;
        for i in 0..seg {
            self.indices
                .extend_from_slice(&[b, b + 1 + i, b + 1 + (i + 1) % seg]);
        }
        self.emit(seg * 3, io);
    }

    /// Ring (annulus) with uniform colour, useful for hue wheel edges.
    pub fn ring(
        &mut self,
        cx: f32,
        cy: f32,
        r_inner: f32,
        r_outer: f32,
        seg: u32,
        color: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            let (s, c_a) = a.sin_cos();
            self.vertices.push(Vertex {
                pos: [cx + r_inner * c_a, cy + r_inner * s],
                uv: WHITE_UV,
                color,
            });
            self.vertices.push(Vertex {
                pos: [cx + r_outer * c_a, cy + r_outer * s],
                uv: WHITE_UV,
                color,
            });
        }
        let io = self.indices.len() as u32;
        let mut count = 0u32;
        for i in 0..seg {
            let i0 = i * 2;
            let i1 = ((i + 1) % seg) * 2;
            self.indices.extend_from_slice(&[
                b + i0,
                b + i0 + 1,
                b + i1 + 1,
                b + i0,
                b + i1 + 1,
                b + i1,
            ]);
            count += 6;
        }
        self.emit(count, io);
    }

    // Anti-aliased primitives
    //
    // Each shape emits an extra ring of vertices whose alpha fades
    // to zero over a 1 px fringe.  The GPU hardware-interpolates the
    // alpha across the fringe, producing smooth edges.

    /// Anti-aliased rounded rectangle.
    ///
    /// This is the workhorse primitive for cards, key caps, buttons,
    /// slider tracks, and every other rounded UI element.
    pub fn aa_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        color: [f32; 4],
    ) {
        if w < 1.0 || h < 1.0 {
            return;
        }
        let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
        if r < 0.5 {
            return self.aa_rect(x, y, w, h, color);
        }

        let seg = AA_CORNER_SEG;
        let aa = AA_FRINGE;
        let transparent = with_zero_alpha(color);

        let corners: [(f32, f32, f32, f32); 4] = [
            (x + r, y + r, PI, PI * 1.5),
            (x + w - r, y + r, PI * 1.5, PI * 2.0),
            (x + w - r, y + h - r, 0.0, PI * 0.5),
            (x + r, y + h - r, PI * 0.5, PI),
        ];

        let n = 4 * (seg + 1);
        let b = self.vertices.len() as u32;

        // Centre of the fan
        self.vertices.push(Vertex {
            pos: [x + w * 0.5, y + h * 0.5],
            uv: WHITE_UV,
            color,
        });

        // Inner contour (on the shape boundary)
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                let (sa, ca) = a.sin_cos();
                self.vertices.push(Vertex {
                    pos: [cx + r * ca, cy + r * sa],
                    uv: WHITE_UV,
                    color,
                });
            }
        }

        // Outer fringe (expanded outward, fully transparent)
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                let (sa, ca) = a.sin_cos();
                self.vertices.push(Vertex {
                    pos: [cx + (r + aa) * ca, cy + (r + aa) * sa],
                    uv: WHITE_UV,
                    color: transparent,
                });
            }
        }

        let center = b;
        let inner = b + 1;
        let outer = inner + n;
        let io = self.indices.len() as u32;

        // Interior fill (fan from centre to inner contour)
        for i in 0..n {
            let next = (i + 1) % n;
            self.indices
                .extend_from_slice(&[center, inner + i, inner + next]);
        }

        // Fringe strip (inner contour -> outer contour)
        for i in 0..n {
            let next = (i + 1) % n;
            self.indices.extend_from_slice(&[
                inner + i,
                outer + i,
                outer + next,
                inner + i,
                outer + next,
                inner + next,
            ]);
        }

        self.emit(n * 3 + n * 6, io);
    }

    /// Anti-aliased rounded rectangle with a vertical colour gradient.
    ///
    /// Vertex colour is linearly interpolated between `top` (at `y`)
    /// and `bot` (at `y + h`).  Useful for key caps, card headers,
    /// and other elements that benefit from subtle depth shading.
    pub fn aa_rounded_rect_gradient_v(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        top: [f32; 4],
        bot: [f32; 4],
    ) {
        if w < 1.0 || h < 1.0 {
            return;
        }
        let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
        let seg = AA_CORNER_SEG;
        let aa = AA_FRINGE;
        let inv_h = if h > 0.001 { 1.0 / h } else { 0.0 };

        let corners: [(f32, f32, f32, f32); 4] = [
            (x + r, y + r, PI, PI * 1.5),
            (x + w - r, y + r, PI * 1.5, PI * 2.0),
            (x + w - r, y + h - r, 0.0, PI * 0.5),
            (x + r, y + h - r, PI * 0.5, PI),
        ];

        let n = 4 * (seg + 1);
        let b = self.vertices.len() as u32;

        // Centre vertex at midpoint colour
        let mid = lerp_color(top, bot, 0.5);
        self.vertices.push(Vertex {
            pos: [x + w * 0.5, y + h * 0.5],
            uv: WHITE_UV,
            color: mid,
        });

        // Inner contour
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                let (sa, ca) = a.sin_cos();
                let py = cy + r * sa;
                let t = ((py - y) * inv_h).clamp(0.0, 1.0);
                self.vertices.push(Vertex {
                    pos: [cx + r * ca, py],
                    uv: WHITE_UV,
                    color: lerp_color(top, bot, t),
                });
            }
        }

        // Outer fringe
        for &(cx, cy, a0, a1) in &corners {
            for i in 0..=seg {
                let a = a0 + (a1 - a0) * i as f32 / seg as f32;
                let (sa, ca) = a.sin_cos();
                let py = cy + (r + aa) * sa;
                let t = ((py - y) * inv_h).clamp(0.0, 1.0);
                self.vertices.push(Vertex {
                    pos: [cx + (r + aa) * ca, py],
                    uv: WHITE_UV,
                    color: with_zero_alpha(lerp_color(top, bot, t)),
                });
            }
        }

        let center = b;
        let inner = b + 1;
        let outer = inner + n;
        let io = self.indices.len() as u32;

        for i in 0..n {
            let next = (i + 1) % n;
            self.indices
                .extend_from_slice(&[center, inner + i, inner + next]);
        }
        for i in 0..n {
            let next = (i + 1) % n;
            self.indices.extend_from_slice(&[
                inner + i,
                outer + i,
                outer + next,
                inner + i,
                outer + next,
                inner + next,
            ]);
        }

        self.emit(n * 3 + n * 6, io);
    }

    /// Anti-aliased axis-aligned rectangle (radius = 0).
    pub fn aa_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let aa = AA_FRINGE;
        let transparent = with_zero_alpha(color);
        let b = self.vertices.len() as u32;

        // Inner 4 corners
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x, y], uv: WHITE_UV, color },
            Vertex { pos: [x + w, y], uv: WHITE_UV, color },
            Vertex { pos: [x + w, y + h], uv: WHITE_UV, color },
            Vertex { pos: [x, y + h], uv: WHITE_UV, color },
        ]);
        // Outer 4 corners
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x - aa, y - aa], uv: WHITE_UV, color: transparent },
            Vertex { pos: [x + w + aa, y - aa], uv: WHITE_UV, color: transparent },
            Vertex { pos: [x + w + aa, y + h + aa], uv: WHITE_UV, color: transparent },
            Vertex { pos: [x - aa, y + h + aa], uv: WHITE_UV, color: transparent },
        ]);

        let io = self.indices.len() as u32;
        // Fill
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        // Fringe quads (top, right, bottom, left)
        self.indices
            .extend_from_slice(&[b, b + 4, b + 5, b, b + 5, b + 1]);
        self.indices
            .extend_from_slice(&[b + 1, b + 5, b + 6, b + 1, b + 6, b + 2]);
        self.indices
            .extend_from_slice(&[b + 2, b + 6, b + 7, b + 2, b + 7, b + 3]);
        self.indices
            .extend_from_slice(&[b + 3, b + 7, b + 4, b + 3, b + 4, b]);
        self.emit(6 + 24, io);
    }

    /// Anti-aliased filled circle.
    pub fn aa_circle(&mut self, cx: f32, cy: f32, r: f32, color: [f32; 4]) {
        if r < 0.25 {
            return;
        }
        let seg = CIRCLE_SEG;
        let aa = AA_FRINGE;
        let transparent = with_zero_alpha(color);

        let b = self.vertices.len() as u32;
        self.vertices.push(Vertex {
            pos: [cx, cy],
            uv: WHITE_UV,
            color,
        });

        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            let (sa, ca) = a.sin_cos();
            self.vertices.push(Vertex {
                pos: [cx + r * ca, cy + r * sa],
                uv: WHITE_UV,
                color,
            });
        }
        for i in 0..seg {
            let a = 2.0 * PI * i as f32 / seg as f32;
            let (sa, ca) = a.sin_cos();
            self.vertices.push(Vertex {
                pos: [cx + (r + aa) * ca, cy + (r + aa) * sa],
                uv: WHITE_UV,
                color: transparent,
            });
        }

        let inner = b + 1;
        let outer = inner + seg;
        let io = self.indices.len() as u32;

        for i in 0..seg {
            let next = (i + 1) % seg;
            self.indices
                .extend_from_slice(&[b, inner + i, inner + next]);
        }
        for i in 0..seg {
            let next = (i + 1) % seg;
            self.indices.extend_from_slice(&[
                inner + i,
                outer + i,
                outer + next,
                inner + i,
                outer + next,
                inner + next,
            ]);
        }
        self.emit(seg * 3 + seg * 6, io);
    }

    // Per-vertex-colour primitives

    /// Single triangle with per-vertex colours.
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
        self.indices.extend_from_slice(&[b, b + 1, b + 2]);
        self.emit(3, io);
    }

    /// Quad with per-vertex colours (two triangles).
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
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        self.emit(6, io);
    }

    /// Ring segment with per-vertex colours, used for the HSV hue wheel.
    ///
    /// `o0, o1` are the outer edge endpoints and `i0, i1` are the
    /// inner edge endpoints.  `c0` and `c1` give the two hue colours.
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
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        self.emit(6, io);
    }

    /// Horizontal gradient rectangle (left colour -> right colour).
    pub fn gradient_rect(
        &mut self,
        x: f32, y: f32, w: f32, h: f32,
        left: [f32; 4], right: [f32; 4],
    ) {
        self.quad_colors(
            [x, y], left,
            [x + w, y], right,
            [x + w, y + h], right,
            [x, y + h], left,
        );
    }

    // Text rendering

    /// Emit a textured quad for a single font glyph.
    ///
    /// `u0, v0, u1, v1` are normalised atlas coordinates.
    pub fn glyph(
        &mut self,
        x: f32, y: f32, w: f32, h: f32,
        u0: f32, v0: f32, u1: f32, v1: f32,
        color: [f32; 4],
    ) {
        let b = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex { pos: [x, y], uv: [u0, v0], color },
            Vertex { pos: [x + w, y], uv: [u1, v0], color },
            Vertex { pos: [x + w, y + h], uv: [u1, v1], color },
            Vertex { pos: [x, y + h], uv: [u0, v1], color },
        ]);
        let io = self.indices.len() as u32;
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        self.emit(6, io);
    }
}