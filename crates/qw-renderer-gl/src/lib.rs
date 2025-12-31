use qw_common::{BspRender, FaceVertex, Palette, Vec3};

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
    pub lightmap_coords: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderSurface {
    pub vertices: Vec<RenderVertex>,
    pub indices: Vec<u32>,
    pub texture_index: Option<usize>,
    pub texture_name: Option<String>,
    pub lightmap: Option<RenderLightmap>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub mips: [Vec<u8>; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderLightmap {
    pub width: u32,
    pub height: u32,
    pub styles: Vec<u8>,
    pub samples: Vec<Vec<u8>>,
}

impl RenderLightmap {
    pub fn combined_samples(&self, lightstyles: &[String], time: f32) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_combined_samples(lightstyles, time, &mut out);
        out
    }

    pub fn write_combined_samples(&self, lightstyles: &[String], time: f32, out: &mut Vec<u8>) {
        let size = (self.width * self.height) as usize;
        if out.len() != size {
            out.clear();
            out.resize(size, 0);
        } else {
            out.fill(0);
        }

        for (style_index, style_id) in self.styles.iter().enumerate() {
            let scale = style_value(lightstyles, *style_id, time);
            let samples = match self.samples.get(style_index) {
                Some(samples) => samples,
                None => continue,
            };
            for (idx, value) in samples.iter().take(size).enumerate() {
                let sum = out[idx] as f32 + *value as f32 * scale;
                out[idx] = sum.min(255.0).round() as u8;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LightmapBasis {
    min_s: f32,
    min_t: f32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderWorld {
    pub map_name: String,
    pub bsp: BspRender,
    pub surfaces: Vec<RenderSurface>,
    pub textures: Vec<RenderTexture>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuTexture {
    pub width: u32,
    pub height: u32,
    pub mips: [Vec<u8>; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuLightmap {
    pub width: u32,
    pub height: u32,
    pub samples: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuSurface {
    pub vertices: Vec<RenderVertex>,
    pub indices: Vec<u32>,
    pub texture_index: Option<usize>,
    pub lightmap_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GpuWorld {
    pub textures: Vec<GpuTexture>,
    pub lightmaps: Vec<GpuLightmap>,
    pub surfaces: Vec<GpuSurface>,
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
        let (textures, texture_map) = build_textures(&bsp, palette);
        let surfaces = build_surfaces(&bsp, &texture_map);
        Self {
            map_name: map_name.into(),
            bsp,
            surfaces,
            textures,
        }
    }
}

fn build_gpu_world(world: &RenderWorld) -> GpuWorld {
    let textures = world
        .textures
        .iter()
        .map(|texture| GpuTexture {
            width: texture.width,
            height: texture.height,
            mips: texture.mips.clone(),
        })
        .collect::<Vec<_>>();

    let mut lightmaps = Vec::new();
    let mut surfaces = Vec::with_capacity(world.surfaces.len());

    for surface in &world.surfaces {
        let lightmap_index = surface.lightmap.as_ref().map(|lightmap| {
            let mut samples = Vec::new();
            lightmap.write_combined_samples(&[], 0.0, &mut samples);
            let index = lightmaps.len();
            lightmaps.push(GpuLightmap {
                width: lightmap.width,
                height: lightmap.height,
                samples,
            });
            index
        });

        surfaces.push(GpuSurface {
            vertices: surface.vertices.clone(),
            indices: surface.indices.clone(),
            texture_index: surface.texture_index,
            lightmap_index,
        });
    }

    GpuWorld {
        textures,
        lightmaps,
        surfaces,
    }
}

#[derive(Debug, Clone)]
pub struct GlRenderer {
    config: RendererConfig,
    frame_index: u64,
    last_view: Option<RenderView>,
    last_world: Option<RenderWorld>,
    gpu_world: Option<GpuWorld>,
}

impl GlRenderer {
    pub fn new(config: RendererConfig) -> Self {
        Self {
            config,
            frame_index: 0,
            last_view: None,
            last_world: None,
            gpu_world: None,
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
        self.gpu_world = Some(build_gpu_world(&world));
        self.last_world = Some(world);
    }

    pub fn world(&self) -> Option<&RenderWorld> {
        self.last_world.as_ref()
    }

    pub fn gpu_world(&self) -> Option<&GpuWorld> {
        self.gpu_world.as_ref()
    }

    pub fn update_lightmaps(&mut self, lightstyles: &[String], time: f32) {
        let (Some(world), Some(gpu_world)) = (&self.last_world, &mut self.gpu_world) else {
            return;
        };

        for (surface_index, surface) in world.surfaces.iter().enumerate() {
            let Some(lightmap) = surface.lightmap.as_ref() else {
                continue;
            };
            let Some(lightmap_index) = gpu_world
                .surfaces
                .get(surface_index)
                .and_then(|gpu_surface| gpu_surface.lightmap_index)
            else {
                continue;
            };
            let Some(gpu_lightmap) = gpu_world.lightmaps.get_mut(lightmap_index) else {
                continue;
            };
            gpu_lightmap.width = lightmap.width;
            gpu_lightmap.height = lightmap.height;
            lightmap.write_combined_samples(lightstyles, time, &mut gpu_lightmap.samples);
        }
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

fn build_surfaces(bsp: &BspRender, texture_map: &[Option<usize>]) -> Vec<RenderSurface> {
    let mut surfaces = Vec::new();
    for (index, face) in bsp.faces.iter().enumerate() {
        let verts = match bsp.face_vertices(index) {
            Some(verts) if verts.len() >= 3 => verts,
            _ => continue,
        };
        let tex_scale = texture_scale_for_face(bsp, face);
        let lightmap_basis = lightmap_basis(&verts);
        let lightmap = build_lightmap(bsp, face, lightmap_basis.as_ref());
        let lightmap_basis = if lightmap.is_some() {
            lightmap_basis
        } else {
            None
        };
        let vertices = verts
            .into_iter()
            .map(|vertex| RenderVertex {
                position: vertex.position,
                tex_coords: if let Some((width, height)) = tex_scale {
                    [vertex.tex_coords[0] / width, vertex.tex_coords[1] / height]
                } else {
                    vertex.tex_coords
                },
                lightmap_coords: if let Some(basis) = lightmap_basis {
                    [
                        (vertex.tex_coords[0] - basis.min_s) / 16.0,
                        (vertex.tex_coords[1] - basis.min_t) / 16.0,
                    ]
                } else {
                    [0.0, 0.0]
                },
            })
            .collect::<Vec<_>>();

        let texture_name = texture_name_for_face(bsp, face);
        let texture_index = texture_index_for_face(bsp, face, texture_map);
        let indices = triangulate_fan(vertices.len());
        surfaces.push(RenderSurface {
            vertices,
            indices,
            texture_index,
            texture_name,
            lightmap,
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

fn build_textures(
    bsp: &BspRender,
    palette: Option<&Palette>,
) -> (Vec<RenderTexture>, Vec<Option<usize>>) {
    let mut texture_map = vec![None; bsp.textures.len()];
    let Some(palette) = palette else {
        return (Vec::new(), texture_map);
    };

    let mut textures = Vec::new();
    for (index, texture) in bsp.textures.iter().enumerate() {
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
        texture_map[index] = Some(textures.len() - 1);
    }

    (textures, texture_map)
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

fn texture_index_for_face(
    bsp: &BspRender,
    face: &qw_common::Face,
    texture_map: &[Option<usize>],
) -> Option<usize> {
    let texinfo = bsp.texinfo.get(face.texinfo as usize)?;
    if texinfo.texture_id < 0 {
        return None;
    }
    let index = texinfo.texture_id as usize;
    texture_map.get(index).and_then(|slot| *slot)
}

fn texture_scale_for_face(bsp: &BspRender, face: &qw_common::Face) -> Option<(f32, f32)> {
    let texinfo = bsp.texinfo.get(face.texinfo as usize)?;
    if texinfo.texture_id < 0 {
        return None;
    }
    let texture = bsp.textures.get(texinfo.texture_id as usize)?;
    if texture.width == 0 || texture.height == 0 {
        return None;
    }
    Some((texture.width as f32, texture.height as f32))
}

fn build_lightmap(
    bsp: &BspRender,
    face: &qw_common::Face,
    basis: Option<&LightmapBasis>,
) -> Option<RenderLightmap> {
    if face.light_ofs < 0 {
        return None;
    }
    if bsp.lighting.is_empty() {
        return None;
    }

    let basis = basis?;
    let (width, height) = (basis.width, basis.height);
    let size = (width * height) as usize;
    if size == 0 {
        return None;
    }

    let styles: Vec<u8> = face
        .styles
        .iter()
        .copied()
        .filter(|style| *style != 255)
        .collect();
    if styles.is_empty() {
        return None;
    }

    let offset = face.light_ofs as usize;
    let total = size
        .checked_mul(styles.len())
        .and_then(|count| offset.checked_add(count))?;
    if total > bsp.lighting.len() {
        return None;
    }

    let mut samples = Vec::with_capacity(styles.len());
    for idx in 0..styles.len() {
        let start = offset + idx * size;
        let end = start + size;
        samples.push(bsp.lighting[start..end].to_vec());
    }

    Some(RenderLightmap {
        width,
        height,
        styles,
        samples,
    })
}

fn lightmap_basis(verts: &[FaceVertex]) -> Option<LightmapBasis> {
    let first = verts.first()?;
    let mut min_s = first.tex_coords[0];
    let mut max_s = first.tex_coords[0];
    let mut min_t = first.tex_coords[1];
    let mut max_t = first.tex_coords[1];

    for vert in verts.iter().skip(1) {
        min_s = min_s.min(vert.tex_coords[0]);
        max_s = max_s.max(vert.tex_coords[0]);
        min_t = min_t.min(vert.tex_coords[1]);
        max_t = max_t.max(vert.tex_coords[1]);
    }

    let min_s = (min_s / 16.0).floor() * 16.0;
    let max_s = (max_s / 16.0).ceil() * 16.0;
    let min_t = (min_t / 16.0).floor() * 16.0;
    let max_t = (max_t / 16.0).ceil() * 16.0;

    let extent_s = max_s - min_s;
    let extent_t = max_t - min_t;
    if extent_s < 0.0 || extent_t < 0.0 {
        return None;
    }

    let width = (extent_s / 16.0).round() as i32 + 1;
    let height = (extent_t / 16.0).round() as i32 + 1;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(LightmapBasis {
        min_s,
        min_t,
        width: width as u32,
        height: height as u32,
    })
}

fn style_value(lightstyles: &[String], style_id: u8, time: f32) -> f32 {
    let style = lightstyles
        .get(style_id as usize)
        .map(|value| value.as_str())
        .unwrap_or("");
    if style.is_empty() {
        return 1.0;
    }
    let index = ((time * 10.0) as usize) % style.len();
    let byte = style.as_bytes()[index];
    if byte < b'a' {
        return 1.0;
    }
    (byte.saturating_sub(b'a') as f32) / 25.0
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
    fn builds_gpu_world_lightmaps() {
        let mut lighting = Vec::new();
        lighting.extend_from_slice(&[5u8; 9]);

        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(32.0, 0.0, 0.0),
                Vec3::new(32.0, 32.0, 0.0),
                Vec3::new(0.0, 32.0, 0.0),
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
                styles: [0, 255, 255, 255],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 32,
                height: 32,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting,
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.set_world(world);
        let gpu_world = renderer.gpu_world().unwrap();
        assert_eq!(gpu_world.lightmaps.len(), 1);
        assert_eq!(gpu_world.surfaces[0].lightmap_index, Some(0));
        assert_eq!(gpu_world.lightmaps[0].samples.len(), 9);
    }

    #[test]
    fn updates_gpu_lightmaps_from_styles() {
        let mut lighting = Vec::new();
        lighting.extend_from_slice(&[100u8; 9]);

        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(32.0, 0.0, 0.0),
                Vec3::new(32.0, 32.0, 0.0),
                Vec3::new(0.0, 32.0, 0.0),
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
                styles: [0, 255, 255, 255],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 32,
                height: 32,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting,
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let mut renderer = GlRenderer::new(RendererConfig::default());
        renderer.set_world(world);

        let mut styles = vec![String::new(); 1];
        styles[0] = "b".to_string();
        renderer.update_lightmaps(&styles, 0.0);

        let gpu_world = renderer.gpu_world().unwrap();
        assert_eq!(gpu_world.lightmaps[0].samples[0], 4);
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
        assert_eq!(world.surfaces[0].texture_index, None);
        assert_eq!(world.surfaces[0].vertices[1].tex_coords, [1.0 / 64.0, 0.0]);
        assert_eq!(world.surfaces[0].vertices[1].lightmap_coords, [0.0, 0.0]);
        assert_eq!(world.surfaces[0].texture_name.as_deref(), Some("wall"));
        assert!(world.surfaces[0].lightmap.is_none());
    }

    #[test]
    fn assigns_texture_indices_from_mips() {
        let mut palette_bytes = vec![0u8; 256 * 3];
        palette_bytes[0] = 10;
        palette_bytes[1] = 20;
        palette_bytes[2] = 30;
        let palette = Palette::from_bytes(&palette_bytes).unwrap();

        let mip = qw_common::MipTexture {
            width: 2,
            height: 2,
            mips: [vec![0; 4], vec![0], vec![0], vec![0]],
        };
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
                width: 2,
                height: 2,
                offsets: [0; 4],
                mip_data: Some(mip),
            }],
            lighting: Vec::new(),
        };

        let world = RenderWorld::from_bsp_with_palette("maps/test.bsp", bsp, Some(&palette));
        assert_eq!(world.textures.len(), 1);
        assert_eq!(world.surfaces[0].texture_index, Some(0));
    }

    #[test]
    fn builds_lightmap_from_face() {
        let mut lighting = Vec::new();
        lighting.extend_from_slice(&[1u8; 9]);

        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(32.0, 0.0, 0.0),
                Vec3::new(32.0, 32.0, 0.0),
                Vec3::new(0.0, 32.0, 0.0),
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
                styles: [0, 255, 255, 255],
                light_ofs: 0,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 32,
                height: 32,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting,
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let lightmap = world.surfaces[0].lightmap.as_ref().unwrap();
        assert_eq!(world.surfaces[0].vertices[1].lightmap_coords, [2.0, 0.0]);
        assert_eq!(lightmap.width, 3);
        assert_eq!(lightmap.height, 3);
        assert_eq!(lightmap.styles, vec![0]);
        assert_eq!(lightmap.samples[0].len(), 9);
        assert_eq!(lightmap.samples[0][0], 1);
    }

    #[test]
    fn combines_lightmap_styles() {
        let lightmap = RenderLightmap {
            width: 1,
            height: 1,
            styles: vec![0],
            samples: vec![vec![50]],
        };
        let mut styles = vec![String::new(); 1];
        styles[0] = "z".to_string();
        let combined = lightmap.combined_samples(&styles, 0.0);
        assert_eq!(combined, vec![50]);
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
