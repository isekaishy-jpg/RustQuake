use qw_common::{
    AliasModel, BspRender, FaceVertex, MdlFrame, MdlSkin, Palette, Sprite, SpriteFrame,
    SpriteImage, Vec3,
};

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
    pub bounds: RenderBounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderBounds {
    pub center: Vec3,
    pub radius: f32,
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
    pub brush_models: Vec<RenderBrushModel>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderBrushModel {
    pub origin: Vec3,
    pub surfaces: Vec<usize>,
    pub bounds: RenderBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderEntityKind {
    Brush,
    Alias,
    Sprite,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderEntity {
    pub kind: RenderEntityKind,
    pub model_index: usize,
    pub origin: Vec3,
    pub angles: Vec3,
    pub frame: u32,
    pub skin: u32,
    pub alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderDrawList {
    pub opaque_surfaces: Vec<usize>,
    pub transparent_surfaces: Vec<usize>,
    pub opaque_entities: Vec<RenderEntity>,
    pub transparent_entities: Vec<RenderEntity>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderModelTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderModelKind {
    Alias(AliasModel),
    Sprite(Sprite),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderModel {
    pub kind: RenderModelKind,
    pub textures: Vec<RenderModelTexture>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderModelFrame<'a> {
    Alias(&'a MdlFrame),
    Sprite(&'a SpriteImage),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedEntity<'a> {
    pub entity: &'a RenderEntity,
    pub model: Option<&'a RenderModel>,
    pub frame: Option<RenderModelFrame<'a>>,
    pub texture_index: Option<usize>,
}

impl RenderModel {
    pub fn frame_at_time(&self, frame: u32, time: f32) -> Option<RenderModelFrame<'_>> {
        match &self.kind {
            RenderModelKind::Alias(model) => model
                .frame_at_time(frame as usize, time)
                .map(RenderModelFrame::Alias),
            RenderModelKind::Sprite(sprite) => sprite
                .frame_at_time(frame as usize, time)
                .map(RenderModelFrame::Sprite),
        }
    }

    pub fn texture_index(&self, frame: u32, skin: u32, time: f32) -> Option<usize> {
        if self.textures.is_empty() {
            return None;
        }
        let index = match &self.kind {
            RenderModelKind::Alias(model) => alias_skin_index(model, skin, time),
            RenderModelKind::Sprite(sprite) => sprite_frame_index(sprite, frame, time),
        }?;
        if index < self.textures.len() {
            Some(index)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UiText {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub color: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiLayer {
    pub texts: Vec<UiText>,
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
        let (surfaces, face_to_surface) = build_surfaces(&bsp, &texture_map);
        let brush_models = build_brush_models(&bsp, &surfaces, &face_to_surface);
        Self {
            map_name: map_name.into(),
            bsp,
            surfaces,
            textures,
            brush_models,
        }
    }
}

pub fn build_draw_list(world: &RenderWorld, entities: &[RenderEntity]) -> RenderDrawList {
    let mut opaque_surfaces = Vec::new();
    let mut transparent_surfaces = Vec::new();
    for (index, surface) in world.surfaces.iter().enumerate() {
        if is_transparent_surface(surface) {
            transparent_surfaces.push(index);
        } else {
            opaque_surfaces.push(index);
        }
    }

    let mut opaque_entities = Vec::new();
    let mut transparent_entities = Vec::new();
    for entity in entities {
        if is_transparent_entity(entity) {
            transparent_entities.push(entity.clone());
        } else {
            opaque_entities.push(entity.clone());
        }
    }

    RenderDrawList {
        opaque_surfaces,
        transparent_surfaces,
        opaque_entities,
        transparent_entities,
    }
}

fn is_transparent_surface(surface: &RenderSurface) -> bool {
    surface
        .texture_name
        .as_deref()
        .is_some_and(|name| name.starts_with('{'))
}

fn is_transparent_entity(entity: &RenderEntity) -> bool {
    entity.alpha < 1.0 || matches!(entity.kind, RenderEntityKind::Sprite)
}
fn build_surfaces(
    bsp: &BspRender,
    texture_map: &[Option<usize>],
) -> (Vec<RenderSurface>, Vec<Option<usize>>) {
    let mut surfaces = Vec::new();
    let mut face_to_surface = vec![None; bsp.faces.len()];
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
        let bounds = bounds_for_vertices(&vertices);
        surfaces.push(RenderSurface {
            vertices,
            indices,
            texture_index,
            texture_name,
            lightmap,
            bounds,
        });
        face_to_surface[index] = Some(surfaces.len() - 1);
    }
    (surfaces, face_to_surface)
}

fn build_brush_models(
    bsp: &BspRender,
    surfaces: &[RenderSurface],
    face_to_surface: &[Option<usize>],
) -> Vec<RenderBrushModel> {
    let mut models = Vec::with_capacity(bsp.models.len());
    for model in &bsp.models {
        let start = model.firstface.max(0) as usize;
        let count = model.numfaces.max(0) as usize;
        let end = start.saturating_add(count).min(face_to_surface.len());
        let mut surface_indices = Vec::new();
        for surface in face_to_surface.iter().take(end).skip(start) {
            if let Some(surface_index) = *surface {
                surface_indices.push(surface_index);
            }
        }
        let mut bounds = bounds_for_surfaces(surfaces, &surface_indices);
        bounds.center = Vec3::new(
            bounds.center.x - model.origin.x,
            bounds.center.y - model.origin.y,
            bounds.center.z - model.origin.z,
        );
        models.push(RenderBrushModel {
            origin: model.origin,
            surfaces: surface_indices,
            bounds,
        });
    }
    models
}

fn bounds_for_surfaces(surfaces: &[RenderSurface], indices: &[usize]) -> RenderBounds {
    let mut min = None::<Vec3>;
    let mut max = None::<Vec3>;
    for surface_index in indices {
        let Some(surface) = surfaces.get(*surface_index) else {
            continue;
        };
        let center = surface.bounds.center;
        let radius = surface.bounds.radius;
        let surface_min = Vec3::new(center.x - radius, center.y - radius, center.z - radius);
        let surface_max = Vec3::new(center.x + radius, center.y + radius, center.z + radius);
        match (min.as_mut(), max.as_mut()) {
            (Some(min), Some(max)) => {
                min.x = min.x.min(surface_min.x);
                min.y = min.y.min(surface_min.y);
                min.z = min.z.min(surface_min.z);
                max.x = max.x.max(surface_max.x);
                max.y = max.y.max(surface_max.y);
                max.z = max.z.max(surface_max.z);
            }
            _ => {
                min = Some(surface_min);
                max = Some(surface_max);
            }
        }
    }

    let (Some(min), Some(max)) = (min, max) else {
        return RenderBounds {
            center: Vec3::default(),
            radius: 0.0,
        };
    };

    let center = Vec3::new(
        (min.x + max.x) * 0.5,
        (min.y + max.y) * 0.5,
        (min.z + max.z) * 0.5,
    );
    let dx = max.x - center.x;
    let dy = max.y - center.y;
    let dz = max.z - center.z;
    RenderBounds {
        center,
        radius: (dx * dx + dy * dy + dz * dz).sqrt(),
    }
}

fn bounds_for_vertices(vertices: &[RenderVertex]) -> RenderBounds {
    let Some(first) = vertices.first() else {
        return RenderBounds {
            center: Vec3::default(),
            radius: 0.0,
        };
    };
    let mut min = first.position;
    let mut max = first.position;
    for vertex in vertices.iter().skip(1) {
        let pos = vertex.position;
        min.x = min.x.min(pos.x);
        min.y = min.y.min(pos.y);
        min.z = min.z.min(pos.z);
        max.x = max.x.max(pos.x);
        max.y = max.y.max(pos.y);
        max.z = max.z.max(pos.z);
    }
    let center = Vec3::new(
        (min.x + max.x) * 0.5,
        (min.y + max.y) * 0.5,
        (min.z + max.z) * 0.5,
    );
    let mut radius_sq: f32 = 0.0;
    for vertex in vertices {
        let dx = vertex.position.x - center.x;
        let dy = vertex.position.y - center.y;
        let dz = vertex.position.z - center.z;
        radius_sq = radius_sq.max(dx * dx + dy * dy + dz * dz);
    }
    RenderBounds {
        center,
        radius: radius_sq.sqrt(),
    }
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

fn alias_skin_index(model: &AliasModel, skin: u32, time: f32) -> Option<usize> {
    let target = skin as usize;
    let mut offset = 0usize;
    for (idx, entry) in model.skins.iter().enumerate() {
        if idx == target {
            let index = match entry {
                MdlSkin::Single { .. } => 0,
                MdlSkin::Group {
                    intervals, frames, ..
                } => group_frame_index(intervals, frames.len(), time),
            };
            return Some(offset + index);
        }
        offset += match entry {
            MdlSkin::Single { .. } => 1,
            MdlSkin::Group { frames, .. } => frames.len(),
        };
    }
    None
}

fn sprite_frame_index(sprite: &Sprite, frame: u32, time: f32) -> Option<usize> {
    let target = frame as usize;
    let mut offset = 0usize;
    for (idx, entry) in sprite.frames.iter().enumerate() {
        if idx == target {
            let index = match entry {
                SpriteFrame::Single(_) => 0,
                SpriteFrame::Group { intervals, frames } => {
                    group_frame_index(intervals, frames.len(), time)
                }
            };
            return Some(offset + index);
        }
        offset += match entry {
            SpriteFrame::Single(_) => 1,
            SpriteFrame::Group { frames, .. } => frames.len(),
        };
    }
    None
}

fn group_frame_index(intervals: &[f32], frame_count: usize, time: f32) -> usize {
    if frame_count == 0 {
        return 0;
    }
    let count = intervals.len().min(frame_count);
    if count == 0 {
        return 0;
    }
    let total: f32 = intervals.iter().take(count).sum();
    if total <= 0.0 {
        return 0;
    }
    let mut t = time % total;
    if t < 0.0 {
        t += total;
    }
    for (idx, interval) in intervals.iter().take(count).enumerate() {
        if t < *interval {
            return idx;
        }
        t -= *interval;
    }
    0
}
#[cfg(test)]
mod tests {
    use super::*;
    use qw_common::{BspModel, SpriteFrame, SpriteHeader, SpriteImage};

    #[test]
    fn resolves_sprite_group_frame_by_time() {
        let frame_a = SpriteImage {
            width: 1,
            height: 1,
            origin: (0, 0),
            pixels: vec![1],
        };
        let frame_b = SpriteImage {
            width: 1,
            height: 1,
            origin: (0, 0),
            pixels: vec![2],
        };
        let sprite = Sprite {
            header: SpriteHeader {
                sprite_type: 0,
                bounding_radius: 0.0,
                width: 1,
                height: 1,
                num_frames: 1,
                beam_length: 0.0,
                sync_type: 0,
            },
            frames: vec![SpriteFrame::Group {
                intervals: vec![0.1, 0.2],
                frames: vec![frame_a, frame_b],
            }],
        };
        let model = RenderModel {
            kind: RenderModelKind::Sprite(sprite),
            textures: Vec::new(),
        };
        let frame = model.frame_at_time(0, 0.15).unwrap();
        match frame {
            RenderModelFrame::Sprite(image) => {
                assert_eq!(image.pixels[0], 2);
            }
            _ => panic!("expected sprite frame"),
        }
    }

    #[test]
    fn selects_alias_skin_texture_index() {
        let header = qw_common::MdlHeader {
            scale: Vec3::default(),
            translate: Vec3::default(),
            bounding_radius: 0.0,
            eye_position: Vec3::default(),
            num_skins: 2,
            skin_width: 1,
            skin_height: 1,
            num_verts: 0,
            num_tris: 0,
            num_frames: 0,
            sync_type: 0,
            flags: 0,
            size: 0.0,
        };
        let model = RenderModel {
            kind: RenderModelKind::Alias(AliasModel {
                header,
                skins: vec![
                    MdlSkin::Single {
                        width: 1,
                        height: 1,
                        indices: vec![0],
                    },
                    MdlSkin::Group {
                        width: 1,
                        height: 1,
                        intervals: vec![0.1, 0.2],
                        frames: vec![vec![1], vec![2]],
                    },
                ],
                tex_coords: Vec::new(),
                triangles: Vec::new(),
                frames: Vec::new(),
            }),
            textures: vec![
                RenderModelTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 0, 255],
                },
                RenderModelTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![1, 1, 1, 255],
                },
                RenderModelTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![2, 2, 2, 255],
                },
            ],
        };

        assert_eq!(model.texture_index(0, 0, 0.0), Some(0));
        assert_eq!(model.texture_index(0, 1, 0.15), Some(2));
    }

    #[test]
    fn selects_sprite_texture_index() {
        let sprite = Sprite {
            header: SpriteHeader {
                sprite_type: 0,
                bounding_radius: 0.0,
                width: 1,
                height: 1,
                num_frames: 1,
                beam_length: 0.0,
                sync_type: 0,
            },
            frames: vec![SpriteFrame::Group {
                intervals: vec![0.1, 0.2],
                frames: vec![
                    SpriteImage {
                        width: 1,
                        height: 1,
                        origin: (0, 0),
                        pixels: vec![1],
                    },
                    SpriteImage {
                        width: 1,
                        height: 1,
                        origin: (0, 0),
                        pixels: vec![2],
                    },
                ],
            }],
        };
        let model = RenderModel {
            kind: RenderModelKind::Sprite(sprite),
            textures: vec![
                RenderModelTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![1, 1, 1, 255],
                },
                RenderModelTexture {
                    width: 1,
                    height: 1,
                    rgba: vec![2, 2, 2, 255],
                },
            ],
        };

        assert_eq!(model.texture_index(0, 0, 0.15), Some(1));
    }

    #[test]
    fn builds_draw_list_with_transparent_surfaces() {
        let bsp = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            surf_edges: vec![0, 1, 2, 3],
            texinfo: vec![
                qw_common::TexInfo {
                    s_vec: Vec3::new(1.0, 0.0, 0.0),
                    s_offset: 0.0,
                    t_vec: Vec3::new(0.0, 1.0, 0.0),
                    t_offset: 0.0,
                    texture_id: 0,
                    flags: 0,
                },
                qw_common::TexInfo {
                    s_vec: Vec3::new(1.0, 0.0, 0.0),
                    s_offset: 0.0,
                    t_vec: Vec3::new(0.0, 1.0, 0.0),
                    t_offset: 0.0,
                    texture_id: 1,
                    flags: 0,
                },
            ],
            faces: vec![
                qw_common::Face {
                    plane_num: 0,
                    side: 0,
                    first_edge: 0,
                    num_edges: 4,
                    texinfo: 0,
                    styles: [255; 4],
                    light_ofs: -1,
                },
                qw_common::Face {
                    plane_num: 0,
                    side: 0,
                    first_edge: 0,
                    num_edges: 4,
                    texinfo: 1,
                    styles: [255; 4],
                    light_ofs: -1,
                },
            ],
            textures: vec![
                qw_common::BspTexture {
                    name: "wall".to_string(),
                    width: 64,
                    height: 64,
                    offsets: [0; 4],
                    mip_data: None,
                },
                qw_common::BspTexture {
                    name: "{water".to_string(),
                    width: 64,
                    height: 64,
                    offsets: [0; 4],
                    mip_data: None,
                },
            ],
            lighting: Vec::new(),
            models: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let draw_list = build_draw_list(&world, &[]);
        assert_eq!(draw_list.opaque_surfaces, vec![0]);
        assert_eq!(draw_list.transparent_surfaces, vec![1]);
    }

    #[test]
    fn builds_draw_list_with_transparent_entities() {
        let entities = vec![
            RenderEntity {
                kind: RenderEntityKind::Alias,
                model_index: 1,
                origin: Vec3::default(),
                angles: Vec3::default(),
                frame: 0,
                skin: 0,
                alpha: 1.0,
            },
            RenderEntity {
                kind: RenderEntityKind::Sprite,
                model_index: 2,
                origin: Vec3::default(),
                angles: Vec3::default(),
                frame: 0,
                skin: 0,
                alpha: 1.0,
            },
        ];

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
                models: Vec::new(),
            },
        );
        let draw_list = build_draw_list(&world, &entities);
        assert_eq!(draw_list.opaque_entities.len(), 1);
        assert_eq!(draw_list.transparent_entities.len(), 1);
        assert_eq!(draw_list.transparent_entities[0].model_index, 2);
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
            models: Vec::new(),
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
    fn builds_brush_models_from_faces() {
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
            faces: vec![
                qw_common::Face {
                    plane_num: 0,
                    side: 0,
                    first_edge: 0,
                    num_edges: 4,
                    texinfo: 0,
                    styles: [0; 4],
                    light_ofs: -1,
                },
                qw_common::Face {
                    plane_num: 0,
                    side: 0,
                    first_edge: 0,
                    num_edges: 4,
                    texinfo: 0,
                    styles: [0; 4],
                    light_ofs: -1,
                },
            ],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 1,
                height: 1,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting: Vec::new(),
            models: vec![
                BspModel {
                    mins: Vec3::default(),
                    maxs: Vec3::default(),
                    origin: Vec3::default(),
                    headnode: [0; qw_common::MAX_MAP_HULLS],
                    visleafs: 0,
                    firstface: 0,
                    numfaces: 1,
                },
                BspModel {
                    mins: Vec3::default(),
                    maxs: Vec3::default(),
                    origin: Vec3::new(4.0, 5.0, 6.0),
                    headnode: [0; qw_common::MAX_MAP_HULLS],
                    visleafs: 0,
                    firstface: 1,
                    numfaces: 1,
                },
            ],
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        assert_eq!(world.brush_models.len(), 2);
        assert_eq!(world.brush_models[0].surfaces, vec![0]);
        assert_eq!(world.brush_models[1].surfaces, vec![1]);
        assert_eq!(world.brush_models[1].origin, Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn computes_surface_bounds() {
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
                light_ofs: -1,
            }],
            textures: vec![qw_common::BspTexture {
                name: "wall".to_string(),
                width: 1,
                height: 1,
                offsets: [0; 4],
                mip_data: None,
            }],
            lighting: Vec::new(),
            models: Vec::new(),
        };

        let world = RenderWorld::from_bsp("maps/test.bsp", bsp);
        let bounds = world.surfaces[0].bounds;
        assert_eq!(bounds.center, Vec3::new(0.5, 0.5, 0.0));
        let expected = (0.5f32 * 0.5 + 0.5f32 * 0.5).sqrt();
        assert!((bounds.radius - expected).abs() < 1e-6);
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
            models: Vec::new(),
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
            models: Vec::new(),
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
            models: Vec::new(),
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
            models: Vec::new(),
        };

        let world = RenderWorld::from_bsp_with_palette("maps/test.bsp", bsp, Some(&palette));
        let rgba = &world.textures[0].mips[0];
        assert_eq!(&rgba[0..4], &[10, 20, 30, 255]);
        assert_eq!(&rgba[4..8], &[0, 0, 0, 0]);
    }
}
