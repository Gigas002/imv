#[cfg(test)]
mod tests;

pub struct ViewportState {
    pub scale: f32,
    pub offset: (f32, f32),
    pub rotation: u16,
}

impl Default for ViewportState {
    fn default() -> Self {
        ViewportState {
            scale: 1.0,
            offset: (0.0, 0.0),
            rotation: 0,
        }
    }
}

impl ViewportState {
    pub fn zoom_by(&mut self, delta: f32, min_scale: f32, max_scale: f32) {
        self.scale = (self.scale + delta).clamp(min_scale, max_scale);
    }

    pub fn rotate_left(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
    }

    pub fn rotate_right(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.offset.0 += dx;
        self.offset.1 += dy;
    }

    pub fn reset(&mut self) {
        self.scale = 1.0;
        self.offset = (0.0, 0.0);
        self.rotation = 0;
    }
}
