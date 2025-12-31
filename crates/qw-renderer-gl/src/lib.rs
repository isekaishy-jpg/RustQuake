use qw_common::{BspRender, Palette, Vec3};

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
    pub indices: Vec<u32>,
    pub texture_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub mips: [Vec<u8>; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderWorld {
    pub map_name: String,
    pub bsp: BspRender,
    pub surfaces: Vec<RenderSurface>,
    pub textures: Vec<RenderTexture>,
}

impl RenderWorld {
    pub fn from_bsp(map_name: impl Into<String>, bsp: BspRender) -> Self {
        Self::from_bsp_with_palette(map_name, bsp, None)
    }

    pub fn from_bsp_with_palette(
        map_name: impl Into<String>,
        bsp: BspRender,
        palette: Option<&Palette>,
    ) -> Self {
        let surfaces = build_surfaces(&bsp);
        let textures = build_textures(&bsp, palette);
        Self {
            map_name: map_name.into(),
            bsp,
            surfaces,
            textures,
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
        let indices = triangulate_fan(vertices.len());
        surfaces.push(RenderSurface {
            vertices,
            indices,
            texture_name,
        });
    }
    surfaces
}

fn triangulate_fan(vertex_count: usize) -> Vec<u32> {
    if vertex_count < 3 {
        return Vec::new();
    }

    let mut indices = Vec::with_capacity((vertex_count - 2) * 3);
    for i in 1..(vertex_count - 1) {
        indices.push(0);
        indices.push(i as u32);
        indices.push((i + 1) as u32);
    }
    indices
}

fn build_textures(bsp: &BspRender, palette: Option<&Palette>) -> Vec<RenderTexture> {
    let Some(palette) = palette else {
        return Vec::new();
    };

    let mut textures = Vec::new();
    for texture in &bsp.textures {
        let Some(mip) = texture.mip_data.as_ref() else {
            continue;
        };
        if texture.name.is_empty() {
            continue;
        }

        let transparent_index = if texture.name.starts_with('{') {
            Some(255)
        } else {
            None
        };
        let mips = std::array::from_fn(|level| {
            palette.expand_indices(&mip.mips[level], transparent_index)
        });
        textures.push(RenderTexture {
            name: texture.name.clone(),
            width: mip.width,
            height: mip.height,
            mips,
        });
    }

    textures
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
                mip_data: None,
            }],
            lighting: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        assert_eq!(world.surfaces.len(), 1);
        assert_eq!(world.surfaces[0].vertices.len(), 4);
        assert_eq!(world.surfaces[0].indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(world.surfaces[0].texture_name.as_deref(), Some("wall"));
    }

    #[test]
    fn builds_textures_from_palette() {
        let mut palette_bytes = vec![0u8; 256 * 3];
        palette_bytes[0] = 10;
        palette_bytes[1] = 20;
        palette_bytes[2] = 30;
        let palette = Palette::from_bytes(&palette_bytes).unwrap();

        let mip = qw_common::MipTexture {
            width: 1,
            height: 1,
            mips: [vec![0], vec![0], vec![0], vec![0]],
        };
        let bsp = BspRender {
            vertices: Vec::new(),
            edges: Vec::new(),
            surf_edges: Vec::new(),
            texinfo: Vec::new(),
            faces: Vec::new(),
            textures: vec![qw_common::BspTexture {
                name: "brick".to_string(),
                width: 1,
                height: 1,
                offsets: [0; 4],
                mip_data: Some(mip),
            }],
            lighting: Vec::new(),
        };

        let world = RenderWorld::from_bsp_with_palette("maps/test.bsp", bsp, Some(&palette));
        assert_eq!(world.textures.len(), 1);
        assert_eq!(world.textures[0].mips[0], vec![10, 20, 30, 255]);
    }

    #[test]
    fn applies_transparent_index_for_brace_textures() {
        let mut palette_bytes = vec![0u8; 256 * 3];
        palette_bytes[0] = 10;
        palette_bytes[1] = 20;
        palette_bytes[2] = 30;
        let base = 255 * 3;
        palette_bytes[base] = 40;
        palette_bytes[base + 1] = 50;
        palette_bytes[base + 2] = 60;
        let palette = Palette::from_bytes(&palette_bytes).unwrap();

        let mip = qw_common::MipTexture {
            width: 2,
            height: 2,
            mips: [vec![0, 255, 0, 255], vec![0], vec![0], vec![0]],
        };
        let bsp = BspRender {
            vertices: Vec::new(),
            edges: Vec::new(),
            surf_edges: Vec::new(),
            texinfo: Vec::new(),
            faces: Vec::new(),
            textures: vec![qw_common::BspTexture {
                name: "{water".to_string(),
                width: 2,
                height: 2,
                offsets: [0; 4],
                mip_data: Some(mip),
            }],
            lighting: Vec::new(),
        };

        let world = RenderWorld::from_bsp_with_palette("maps/test.bsp", bsp, Some(&palette));
        let rgba = &world.textures[0].mips[0];
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0]);
    }
}
