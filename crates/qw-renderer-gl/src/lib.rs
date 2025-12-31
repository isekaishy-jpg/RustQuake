use qw_common::Vec3;

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderView {
    pub origin: Vec3,
    pub angles: Vec3,
    pub fov_y: f32,
}

#[derive(Debug, Clone)]
pub struct GlRenderer {
    config: RendererConfig,
    frame_index: u64,
    last_view: Option<RenderView>,
}

impl GlRenderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            frame_index: 0,
            last_view: None,
        }
    }

    pub fn frame_index(&self) -> u64 {
        self.frame_index
    }

    pub fn set_view(&mut self, view: RenderView) {
        self.last_view = Some(view);
    }

    pub fn view(&self) -> Option<RenderView> {
        self.last_view
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

    #[test]
    fn stores_view_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let view = RenderView {
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(10.0, 20.0, 30.0),
            fov_y: 90.0,
        };
        renderer.set_view(view);
        assert_eq!(renderer.view(), Some(view));
    }
}
