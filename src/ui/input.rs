/// Mouse and window state captured each frame from Win32 messages.
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
    /// True on the frame the left button was released.
    #[inline]
    pub fn clicked(&self) -> bool {
        !self.mouse_down && self.mouse_was_down
    }

    /// True on the frame the left button was first pressed.
    #[inline]
    pub fn just_pressed(&self) -> bool {
        self.mouse_down && !self.mouse_was_down
    }

    /// True on the frame the right button was released.
    #[inline]
    pub fn right_clicked(&self) -> bool {
        !self.right_down && self.right_was_down
    }

    /// Hit-test a rectangle.
    #[inline]
    pub fn in_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_x >= x
            && self.mouse_x < x + w
            && self.mouse_y >= y
            && self.mouse_y < y + h
    }

    /// Call at the end of each frame to latch button states.
    pub fn end_frame(&mut self) {
        self.mouse_was_down = self.mouse_down;
        self.right_was_down = self.right_down;
        self.scroll_delta = 0.0;
        self.resized = false;
    }
}

/// Exponential smoothing.  Returns `target` when the difference
/// is negligible, avoiding endless micro-updates.
#[inline]
pub fn smooth(current: f32, target: f32, speed: f32, dt: f32) -> f32 {
    let d = target - current;
    if d.abs() < 0.001 {
        target
    } else {
        current + d * (speed * dt).min(1.0)
    }
}