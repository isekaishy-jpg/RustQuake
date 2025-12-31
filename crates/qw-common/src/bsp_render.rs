// Render-oriented BSP lump parsing for QuakeWorld maps.

use crate::bsp::{
    Bsp, BspError, LUMP_EDGES, LUMP_FACES, LUMP_LIGHTING, LUMP_SURFEDGES, LUMP_TEXINFO,
    LUMP_TEXTURES, LUMP_VERTEXES,
};
use crate::types::Vec3;

#[derive(Debug, Clone, PartialEq)]
pub struct BspTexture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub offsets: [u32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexInfo {
    pub s_vec: Vec3,
    pub s_offset: f32,
    pub t_vec: Vec3,
    pub t_offset: f32,
    pub texture_id: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Face {
    pub plane_num: u16,
    pub side: u16,
    pub first_edge: i32,
    pub num_edges: u16,
    pub texinfo: u16,
    pub styles: [u8; 4],
    pub light_ofs: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceVertex {
    pub position: Vec3,
    pub tex_coords: [f32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BspRender {
    pub vertices: Vec<Vec3>,
    pub edges: Vec<[u16; 2]>,
    pub surf_edges: Vec<i32>,
    pub texinfo: Vec<TexInfo>,
    pub faces: Vec<Face>,
    pub textures: Vec<BspTexture>,
    pub lighting: Vec<u8>,
}

impl BspRender {
    pub fn from_bsp(bsp: &Bsp) -> Result<Self, BspError> {
        Ok(Self {
            vertices: parse_vertices(bsp.lump_slice(LUMP_VERTEXES)?)?,
            edges: parse_edges(bsp.lump_slice(LUMP_EDGES)?)?,
            surf_edges: parse_surf_edges(bsp.lump_slice(LUMP_SURFEDGES)?)?,
            texinfo: parse_texinfo(bsp.lump_slice(LUMP_TEXINFO)?)?,
            faces: parse_faces(bsp.lump_slice(LUMP_FACES)?)?,
            textures: parse_textures(bsp.lump_slice(LUMP_TEXTURES)?)?,
            lighting: bsp.lump_slice(LUMP_LIGHTING)?.to_vec(),
        })
    }

    pub fn face_vertices(&self, face_index: usize) -> Option<Vec<FaceVertex>> {
        let face = self.faces.get(face_index)?;
        let texinfo = self.texinfo.get(face.texinfo as usize)?;
        let first_edge = face.first_edge;
        if first_edge < 0 {
            return None;
        }
        let first_edge = first_edge as usize;
        let num_edges = face.num_edges as usize;
        if first_edge + num_edges > self.surf_edges.len() {
            return None;
        }

        let mut vertices = Vec::with_capacity(num_edges);
        for surf_edge in &self.surf_edges[first_edge..first_edge + num_edges] {
            let edge_index = *surf_edge;
            let vert_index = if edge_index >= 0 {
                let edge = self.edges.get(edge_index as usize)?;
                edge[0]
            } else {
                let edge = self.edges.get((-edge_index) as usize)?;
                edge[1]
            };
            let position = *self.vertices.get(vert_index as usize)?;
            let s = position.dot(texinfo.s_vec) + texinfo.s_offset;
            let t = position.dot(texinfo.t_vec) + texinfo.t_offset;
            vertices.push(FaceVertex {
                position,
                tex_coords: [s, t],
            });
        }

        Some(vertices)
    }
}

const DVERTEX_SIZE: usize = 12;
const DEDGE_SIZE: usize = 4;
const DSURFEDGE_SIZE: usize = 4;
const DTEXINFO_SIZE: usize = 40;
const DFACE_SIZE: usize = 20;

fn parse_vertices(data: &[u8]) -> Result<Vec<Vec3>, BspError> {
    if !data.len().is_multiple_of(DVERTEX_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut vertices = Vec::with_capacity(data.len() / DVERTEX_SIZE);
    for chunk in data.chunks_exact(DVERTEX_SIZE) {
        let vertex = Vec3::new(
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        );
        vertices.push(vertex);
    }
    Ok(vertices)
}

fn parse_edges(data: &[u8]) -> Result<Vec<[u16; 2]>, BspError> {
    if !data.len().is_multiple_of(DEDGE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut edges = Vec::with_capacity(data.len() / DEDGE_SIZE);
    for chunk in data.chunks_exact(DEDGE_SIZE) {
        let v0 = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
        let v1 = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
        edges.push([v0, v1]);
    }
    Ok(edges)
}

fn parse_surf_edges(data: &[u8]) -> Result<Vec<i32>, BspError> {
    if !data.len().is_multiple_of(DSURFEDGE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut edges = Vec::with_capacity(data.len() / DSURFEDGE_SIZE);
    for chunk in data.chunks_exact(DSURFEDGE_SIZE) {
        edges.push(i32::from_le_bytes(chunk[0..4].try_into().unwrap()));
    }
    Ok(edges)
}

fn parse_texinfo(data: &[u8]) -> Result<Vec<TexInfo>, BspError> {
    if !data.len().is_multiple_of(DTEXINFO_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut out = Vec::with_capacity(data.len() / DTEXINFO_SIZE);
    for chunk in data.chunks_exact(DTEXINFO_SIZE) {
        let s_vec = Vec3::new(
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        );
        let s_offset = f32::from_le_bytes(chunk[12..16].try_into().unwrap());
        let t_vec = Vec3::new(
            f32::from_le_bytes(chunk[16..20].try_into().unwrap()),
            f32::from_le_bytes(chunk[20..24].try_into().unwrap()),
            f32::from_le_bytes(chunk[24..28].try_into().unwrap()),
        );
        let t_offset = f32::from_le_bytes(chunk[28..32].try_into().unwrap());
        let texture_id = i32::from_le_bytes(chunk[32..36].try_into().unwrap());
        let flags = i32::from_le_bytes(chunk[36..40].try_into().unwrap());
        out.push(TexInfo {
            s_vec,
            s_offset,
            t_vec,
            t_offset,
            texture_id,
            flags,
        });
    }
    Ok(out)
}

fn parse_faces(data: &[u8]) -> Result<Vec<Face>, BspError> {
    if !data.len().is_multiple_of(DFACE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut faces = Vec::with_capacity(data.len() / DFACE_SIZE);
    for chunk in data.chunks_exact(DFACE_SIZE) {
        let plane_num = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
        let side = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
        let first_edge = i32::from_le_bytes(chunk[4..8].try_into().unwrap());
        let num_edges = u16::from_le_bytes(chunk[8..10].try_into().unwrap());
        let texinfo = u16::from_le_bytes(chunk[10..12].try_into().unwrap());
        let styles = [chunk[12], chunk[13], chunk[14], chunk[15]];
        let light_ofs = i32::from_le_bytes(chunk[16..20].try_into().unwrap());
        faces.push(Face {
            plane_num,
            side,
            first_edge,
            num_edges,
            texinfo,
            styles,
            light_ofs,
        });
    }
    Ok(faces)
}

fn parse_textures(data: &[u8]) -> Result<Vec<BspTexture>, BspError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if data.len() < 4 {
        return Err(BspError::InvalidLump);
    }
    let count = i32::from_le_bytes(data[0..4].try_into().unwrap());
    if count < 0 {
        return Err(BspError::InvalidLump);
    }
    let count = count as usize;
    let directory_size = 4 + count * 4;
    if data.len() < directory_size {
        return Err(BspError::InvalidLump);
    }

    let mut textures = Vec::with_capacity(count);
    for idx in 0..count {
        let offset = i32::from_le_bytes(data[4 + idx * 4..8 + idx * 4].try_into().unwrap());
        if offset <= 0 {
            textures.push(BspTexture {
                name: String::new(),
                width: 0,
                height: 0,
                offsets: [0; 4],
            });
            continue;
        }
        let offset = offset as usize;
        if offset + 16 + 4 * 6 > data.len() {
            return Err(BspError::InvalidLump);
        }
        let name = parse_texture_name(&data[offset..offset + 16]);
        let width = u32::from_le_bytes(data[offset + 16..offset + 20].try_into().unwrap());
        let height = u32::from_le_bytes(data[offset + 20..offset + 24].try_into().unwrap());
        let mut offsets = [0u32; 4];
        let mut cursor = offset + 24;
        for slot in &mut offsets {
            *slot = u32::from_le_bytes(data[cursor..cursor + 4].try_into().unwrap());
            cursor += 4;
        }
        textures.push(BspTexture {
            name,
            width,
            height,
            offsets,
        });
    }

    Ok(textures)
}

fn parse_texture_name(bytes: &[u8]) -> String {
    let mut out = [0u8; 16];
    let max_len = bytes.len().min(16);
    for i in 0..max_len {
        let mut c = bytes[i];
        if c.is_ascii_uppercase() {
            c += b'a' - b'A';
        }
        out[i] = c;
    }
    let trimmed_len = out.iter().position(|b| *b == 0).unwrap_or(16);
    String::from_utf8_lossy(&out[..trimmed_len]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bsp::{BSP_VERSION, Bsp, HEADER_LUMPS};

    fn build_bsp(lumps: Vec<Vec<u8>>) -> Vec<u8> {
        let header_size = 4 + HEADER_LUMPS * 8;
        let mut data = Vec::new();
        data.extend_from_slice(&BSP_VERSION.to_le_bytes());

        let mut offset = header_size as u32;
        for payload in &lumps {
            let length = payload.len() as u32;
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
            offset += length;
        }

        for payload in lumps {
            data.extend_from_slice(&payload);
        }

        data
    }

    #[test]
    fn parses_texture_names() {
        let mut lumps = vec![Vec::new(); HEADER_LUMPS];
        let mut textures = Vec::new();

        textures.extend_from_slice(&1i32.to_le_bytes());
        textures.extend_from_slice(&8i32.to_le_bytes());

        let mut name = [0u8; 16];
        name[..4].copy_from_slice(b"WALL");
        textures.extend_from_slice(&name);
        textures.extend_from_slice(&64u32.to_le_bytes());
        textures.extend_from_slice(&32u32.to_le_bytes());
        textures.extend_from_slice(&0u32.to_le_bytes());
        textures.extend_from_slice(&0u32.to_le_bytes());
        textures.extend_from_slice(&0u32.to_le_bytes());
        textures.extend_from_slice(&0u32.to_le_bytes());

        lumps[LUMP_TEXTURES] = textures;

        let data = build_bsp(lumps);
        let bsp = Bsp::from_bytes(data).unwrap();
        let render = BspRender::from_bsp(&bsp).unwrap();

        assert_eq!(render.textures.len(), 1);
        assert_eq!(render.textures[0].name, "wall");
        assert_eq!(render.textures[0].width, 64);
        assert_eq!(render.textures[0].height, 32);
    }

    #[test]
    fn builds_face_vertices() {
        let render = BspRender {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(1.0, 1.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
            edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
            surf_edges: vec![0, 1, 2, 3],
            texinfo: vec![TexInfo {
                s_vec: Vec3::new(1.0, 0.0, 0.0),
                s_offset: 0.0,
                t_vec: Vec3::new(0.0, 1.0, 0.0),
                t_offset: 0.0,
                texture_id: 0,
                flags: 0,
            }],
            faces: vec![Face {
                plane_num: 0,
                side: 0,
                first_edge: 0,
                num_edges: 4,
                texinfo: 0,
                styles: [0; 4],
                light_ofs: 0,
            }],
            textures: Vec::new(),
            lighting: Vec::new(),
        };

        let verts = render.face_vertices(0).unwrap();
        assert_eq!(verts.len(), 4);
        assert_eq!(verts[0].position, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(verts[2].position, Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(verts[3].tex_coords, [0.0, 1.0]);
    }
}
