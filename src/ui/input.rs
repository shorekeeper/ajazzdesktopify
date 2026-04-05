/// Mouse/window input state captured each frame from Win32 messages.
#[derive(Default)]
pub struct InputState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
    pub mouse_was_down: bool,
    pub right_down: bool,
    pub right_was_down: bool,
    pub scroll_delta: f32,
    pub width: u32,
    pub height: u32,
    pub resized: bool,
    pub dt: f32,
    pub elapsed: f32,
}

impl InputState {
    #[inline] pub fn clicked(&self) -> bool { !self.mouse_down && self.mouse_was_down }
    #[inline] pub fn just_pressed(&self) -> bool { self.mouse_down && !self.mouse_was_down }
    #[inline] pub fn right_clicked(&self) -> bool { !self.right_down && self.right_was_down }

    #[inline]
    pub fn in_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_x >= x && self.mouse_x < x + w
            && self.mouse_y >= y && self.mouse_y < y + h
    }

    pub fn end_frame(&mut self) {
        self.mouse_was_down = self.mouse_down;
        self.right_was_down = self.right_down;
        self.scroll_delta = 0.0;
        self.resized = false;
    }
}

/// Exponential smoothing for animations. Returns `target` when close enough.
#[inline]
pub fn smooth(current: f32, target: f32, speed: f32, dt: f32) -> f32 {
    let d = target - current;
    if d.abs() < 0.001 { target } else { current + d * (speed * dt).min(1.0) }
}