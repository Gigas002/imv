//! Viewer-side transform state: scale, pan offset, and rotation.
//!
//! [`ViewportState`] is pure data with no I/O or rendering. All mutating
//! methods take explicit min/max parameters so the caller (the binary crate)
//! drives policy from config values without `libimgvwr` knowing about config.

#[cfg(test)]
mod tests;

/// The current pan, zoom, and rotation state for the displayed image.
///
/// Rotation is restricted to multiples of 90°, stored as `0`, `90`, `180`,
/// or `270`. Offset is unconstrained — the image may be panned fully
/// off-screen.
pub struct ViewportState {
    /// Current zoom factor. `1.0` means one image pixel per display pixel.
    pub scale: f32,
    /// Pixel offset from the centred position, `(x, y)`.
    pub offset: (f32, f32),
    /// Clockwise rotation in degrees: `0`, `90`, `180`, or `270`.
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
    /// Adjust zoom by `delta`, clamping to `[min_scale, max_scale]`.
    pub fn zoom_by(&mut self, delta: f32, min_scale: f32, max_scale: f32) {
        self.scale = (self.scale + delta).clamp(min_scale, max_scale);
    }

    /// Rotate 90° counter-clockwise.
    pub fn rotate_left(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
    }

    /// Rotate 90° clockwise.
    pub fn rotate_right(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }

    /// Translate the image by `(dx, dy)` pixels. No clamping — the image can
    /// be dragged fully outside the window area.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.offset.0 += dx;
        self.offset.1 += dy;
    }

    /// Reset to the default state: scale `1.0`, zero offset, no rotation.
    pub fn reset(&mut self) {
        self.scale = 1.0;
        self.offset = (0.0, 0.0);
        self.rotation = 0;
    }
}
