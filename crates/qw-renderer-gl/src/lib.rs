use qw_common::{BspRender, Vec3};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderVertex {
    pub position: Vec3,
    pub tex_coords: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSurface {
    pub vertices: Vec<RenderVertex>,
    pub texture_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderWorld {
    pub map_name: String,
    pub bsp: BspRender,
    pub surfaces: Vec<RenderSurface>,
}

impl RenderWorld {
    pub fn from_bsp(map_name: impl Into<String>, bsp: BspRender) -> Self {
        let surfaces = build_surfaces(&bsp);
        Self {
            map_name: map_name.into(),
            bsp,
            surfaces,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GlRenderer {
    config: RendererConfig,
    frame_index: u64,
    last_view: Option<RenderView>,
    last_world: Option<RenderWorld>,
}

impl GlRenderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            frame_index: 0,
            last_view: None,
            last_world: None,
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

    pub fn set_world(&mut self, world: RenderWorld) {
        self.last_world = Some(world);
    }

    pub fn world(&self) -> Option<&RenderWorld> {
        self.last_world.as_ref()
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

fn build_surfaces(bsp: &BspRender) -> Vec<RenderSurface> {
    let mut surfaces = Vec::new();
    for (index, face) in bsp.faces.iter().enumerate() {
        let verts = match bsp.face_vertices(index) {
            Some(verts) if verts.len() >= 3 => verts,
            _ => continue,
        };
        let vertices = verts
            .into_iter()
            .map(|vertex| RenderVertex {
                position: vertex.position,
                tex_coords: vertex.tex_coords,
            })
            .collect::<Vec<_>>();

        let texture_name = texture_name_for_face(bsp, face);
        surfaces.push(RenderSurface {
            vertices,
            texture_name,
        });
    }
    surfaces
}

fn texture_name_for_face(bsp: &BspRender, face: &qw_common::Face) -> Option<String> {
    let texinfo = bsp.texinfo.get(face.texinfo as usize)?;
    if texinfo.texture_id < 0 {
        return None;
    }
    let index = texinfo.texture_id as usize;
    let texture = bsp.textures.get(index)?;
    if texture.name.is_empty() {
        None
    } else {
        Some(texture.name.clone())
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

    #[test]
    fn stores_world_state() {
        let mut renderer = GlRenderer::new(RendererConfig::default());
        let world = RenderWorld::from_bsp(
            "maps/start.bsp",
            BspRender {
                vertices: Vec::new(),
                edges: Vec::new(),
                surf_edges: Vec::new(),
                texinfo: Vec::new(),
                faces: Vec::new(),
                textures: Vec::new(),
                lighting: Vec::new(),
            },
        );
        renderer.set_world(world.clone());
        assert_eq!(renderer.world(), Some(&world));
    }

    #[test]
    fn builds_surfaces_from_faces() {
        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            surf_edges: vec![0, 1, 2, 3],
            texinfo: vec![qw_common::TexInfo {
                s_vec: Vec3::new(1.0, 0.0, 0.0),
                s_offset: 0.0,
                t_vec: Vec3::new(0.0, 1.0, 0.0),
                t_offset: 0.0,
                texture_id: 0,
                flags: 0,
            }],
            faces: vec![qw_common::Face {
                plane_num: 0,
                side: 0,
                first_edge: 0,
                num_edges: 4,
                texinfo: 0,
                styles: [0; 4],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 64,
                height: 64,
                offsets: [0; 4],
            }],
            lighting: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        assert_eq!(world.surfaces.len(), 1);
        assert_eq!(world.surfaces[0].vertices.len(), 4);
        assert_eq!(world.surfaces[0].texture_name.as_deref(), Some("wall"));
    }
}
