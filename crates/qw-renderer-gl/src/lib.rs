#[derive(Debug, Clone, Copy)]
pub struct RendererConfig {
    pub width: u32,
    pub height: u32,
    pub vsync: bool,
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            vsync: true,
        }
    }
}

pub trait Renderer {
    fn resize(&mut self, width: u32, height: u32);
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn config(&self) -> RendererConfig;
}

#[derive(Debug, Clone)]
pub struct GlRenderer {
    config: RendererConfig,
    frame_index: u64,
}

impl GlRenderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            frame_index: 0,
        }
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }
}

impl Renderer for GlRenderer {
    fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
    }

    fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    fn end_frame(&mut self) {}

    fn config(&self) -> RendererConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resizes_to_nonzero_dimensions() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.resize(0, 0);
        let cfg = renderer.config();
        assert_eq!(cfg.width, 1);
        assert_eq!(cfg.height, 1);
    }

    #[test]
    fn increments_frame_index() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        assert_eq!(renderer.frame_index(), 0);
        renderer.begin_frame();
        assert_eq!(renderer.frame_index(), 1);
    }
}
