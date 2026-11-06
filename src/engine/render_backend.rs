//! Backend seam between the CPU scene and the display.
//!
//! [`RenderBackend`] is the only place the engine may touch a GPU or a window.
//! The headless path uses [`NullBackend`], which validates the scene contract
//! without opening anything; the windowed path uses the wgpu implementation in
//! [`crate::engine::backend_wgpu`].
//!
//! The trait is deliberately narrow: resize and draw. Camera and scene data
//! travel inside [`RenderScene`], so a backend has no simulation state and
//! cannot perturb determinism.

use std::fmt;

use crate::engine::scene::RenderScene;

/// Why a backend could not draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// A texture page could not be decoded or uploaded.
    Texture(String),
    /// The surface could not be acquired or configured.
    Surface(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Texture(detail) => write!(f, "texture page failure: {detail}"),
            Self::Surface(detail) => write!(f, "surface failure: {detail}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// The display seam.
///
/// `draw` owns presentation: it acquires the current surface texture, records
/// the merged scene and presents the frame. `resize` receives physical pixels.
pub trait RenderBackend {
    fn resize(&mut self, width: u32, height: u32);
    fn draw(&mut self, scene: &RenderScene) -> Result<(), RenderError>;
}

/// Headless backend: counts draws and never touches a device or a window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NullBackend {
    draws: u64,
}

impl NullBackend {
    pub const fn new() -> Self {
        Self { draws: 0 }
    }

    /// Frames submitted through [`RenderBackend::draw`].
    pub const fn draws(self) -> u64 {
        self.draws
    }
}

impl RenderBackend for NullBackend {
    fn resize(&mut self, _width: u32, _height: u32) {}

    fn draw(&mut self, _scene: &RenderScene) -> Result<(), RenderError> {
        self.draws = self.draws.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{NullBackend, RenderBackend};
    use crate::engine::scene::RenderScene;

    #[test]
    fn null_backend_counts_every_draw_and_ignores_resize() {
        let mut backend = NullBackend::new();
        let scene = RenderScene::default();
        for _ in 0..3 {
            backend.draw(&scene).expect("headless draw cannot fail");
        }
        backend.resize(1280, 720);
        assert_eq!(backend.draws(), 3);
    }
}
