// BSP header parsing for QuakeWorld maps (version 29).

use std::fmt;

use crate::block_checksum;
use crate::collision::{ClipNode, Hull, Plane};
use crate::types::Vec3;

pub const BSP_VERSION: i32 = 29;
pub const HEADER_LUMPS: usize = 15;

pub const LUMP_ENTITIES: usize = 0;
pub const LUMP_PLANES: usize = 1;
pub const LUMP_TEXTURES: usize = 2;
pub const LUMP_VERTEXES: usize = 3;
pub const LUMP_VISIBILITY: usize = 4;
pub const LUMP_NODES: usize = 5;
pub const LUMP_TEXINFO: usize = 6;
pub const LUMP_FACES: usize = 7;
pub const LUMP_LIGHTING: usize = 8;
pub const LUMP_CLIPNODES: usize = 9;
pub const LUMP_LEAFS: usize = 10;
pub const LUMP_MARKSURFACES: usize = 11;
pub const LUMP_EDGES: usize = 12;
pub const LUMP_SURFEDGES: usize = 13;
pub const LUMP_MODELS: usize = 14;

pub const MAX_MAP_HULLS: usize = 4;

pub const HULL1_MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);
pub const HULL1_MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);
pub const HULL2_MINS: Vec3 = Vec3::new(-32.0, -32.0, -24.0);
pub const HULL2_MAXS: Vec3 = Vec3::new(32.0, 32.0, 64.0);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Lump {
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug)]
pub struct Bsp {
    data: Vec<u8>,
    pub version: i32,
    pub lumps: [Lump; HEADER_LUMPS],
}

#[derive(Debug)]
pub enum BspError {
    InvalidHeader,
    UnsupportedVersion(i32),
    InvalidLump,
}

impl fmt::Display for BspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BspError::InvalidHeader => write!(f, "invalid BSP header"),
            BspError::UnsupportedVersion(v) => write!(f, "unsupported BSP version {}", v),
            BspError::InvalidLump => write!(f, "invalid BSP lump"),
        }
    }
}

impl Bsp {
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, BspError> {
        if data.len() < 4 + HEADER_LUMPS * 8 {
            return Err(BspError::InvalidHeader);
        }

        let version = i32::from_le_bytes(data[0..4].try_into().unwrap());
        if version != BSP_VERSION {
            return Err(BspError::UnsupportedVersion(version));
        }

        let mut lumps = [Lump {
            offset: 0,
            length: 0,
        }; HEADER_LUMPS];
        let mut offset = 4;
        for lump in &mut lumps {
            let ofs = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            let len = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());
            *lump = Lump {
                offset: ofs,
                length: len,
            };
            offset += 8;
        }

        Ok(Bsp {
            data,
            version,
            lumps,
        })
    }

    pub fn lump_slice(&self, index: usize) -> Result<&[u8], BspError> {
        if index >= HEADER_LUMPS {
            return Err(BspError::InvalidLump);
        }
        let lump = self.lumps[index];
        let start = lump.offset as usize;
        let end = start + lump.length as usize;
        if end > self.data.len() {
            return Err(BspError::InvalidLump);
        }
        Ok(&self.data[start..end])
    }

    pub fn entities_text(&self) -> Result<String, BspError> {
        let data = self.lump_slice(LUMP_ENTITIES)?;
        let text = String::from_utf8_lossy(data)
            .trim_end_matches('\0')
            .to_string();
        Ok(text)
    }

    pub fn map_checksums(&self) -> Result<(u32, u32), BspError> {
        let mut checksum = 0u32;
        let mut checksum2 = 0u32;

        for i in 0..HEADER_LUMPS {
            if i == LUMP_ENTITIES {
                continue;
            }
            let data = self.lump_slice(i)?;
            let value = block_checksum(data);
            checksum ^= value;

            if i == LUMP_VISIBILITY || i == LUMP_LEAFS || i == LUMP_NODES {
                continue;
            }
            checksum2 ^= value;
        }

        Ok((checksum, checksum2))
    }
}

#[derive(Debug, Copy, Clone)]
struct BspNode {
    planenum: i32,
    children: [i32; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct BspModel {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub origin: Vec3,
    pub headnode: [i32; MAX_MAP_HULLS],
    pub visleafs: i32,
    pub firstface: i32,
    pub numfaces: i32,
}

#[derive(Debug, Clone)]
pub struct BspCollision {
    pub planes: Vec<Plane>,
    pub clipnodes: Vec<ClipNode>,
    pub hull0_clipnodes: Vec<ClipNode>,
    pub models: Vec<BspModel>,
}

impl BspCollision {
    pub fn from_bsp(bsp: &Bsp) -> Result<Self, BspError> {
        let planes = parse_planes(bsp.lump_slice(LUMP_PLANES)?)?;
        let clipnodes = parse_clipnodes(bsp.lump_slice(LUMP_CLIPNODES)?)?;
        let nodes = parse_nodes(bsp.lump_slice(LUMP_NODES)?)?;
        let leaf_contents = parse_leaf_contents(bsp.lump_slice(LUMP_LEAFS)?)?;
        let models = parse_models(bsp.lump_slice(LUMP_MODELS)?)?;
        let hull0_clipnodes = build_hull0_clipnodes(&nodes, &leaf_contents)?;

        Ok(Self {
            planes,
            clipnodes,
            hull0_clipnodes,
            models,
        })
    }

    pub fn hull(&self, model_index: usize, hull_index: usize) -> Option<Hull<'_>> {
        let model = self.models.get(model_index)?;
        if hull_index >= MAX_MAP_HULLS {
            return None;
        }

        match hull_index {
            0 => {
                if self.hull0_clipnodes.is_empty() {
                    return None;
                }
                let firstclipnode = model.headnode[0];
                let lastclipnode = self.hull0_clipnodes.len() as i32 - 1;
                if firstclipnode < 0 || firstclipnode > lastclipnode {
                    return None;
                }
                Some(Hull {
                    clipnodes: &self.hull0_clipnodes,
                    planes: &self.planes,
                    firstclipnode,
                    lastclipnode,
                    clip_mins: Vec3::default(),
                    clip_maxs: Vec3::default(),
                })
            }
            1 | 2 => {
                if self.clipnodes.is_empty() {
                    return None;
                }
                let firstclipnode = model.headnode[hull_index];
                let lastclipnode = self.clipnodes.len() as i32 - 1;
                if firstclipnode < 0 || firstclipnode > lastclipnode {
                    return None;
                }
                let (clip_mins, clip_maxs) = if hull_index == 1 {
                    (HULL1_MINS, HULL1_MAXS)
                } else {
                    (HULL2_MINS, HULL2_MAXS)
                };
                Some(Hull {
                    clipnodes: &self.clipnodes,
                    planes: &self.planes,
                    firstclipnode,
                    lastclipnode,
                    clip_mins,
                    clip_maxs,
                })
            }
            _ => None,
        }
    }
}

const DPLANE_SIZE: usize = 20;
const DCLIPNODE_SIZE: usize = 8;
const DNODE_SIZE: usize = 24;
const DLEAF_SIZE: usize = 28;
const DMODEL_SIZE: usize = 64;

fn parse_planes(data: &[u8]) -> Result<Vec<Plane>, BspError> {
    if !data.len().is_multiple_of(DPLANE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut planes = Vec::with_capacity(data.len() / DPLANE_SIZE);
    for chunk in data.chunks_exact(DPLANE_SIZE) {
        let normal = Vec3::new(
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        );
        let dist = f32::from_le_bytes(chunk[12..16].try_into().unwrap());
        let plane_type = i32::from_le_bytes(chunk[16..20].try_into().unwrap());
        let mut signbits = 0u8;
        if normal.x < 0.0 {
            signbits |= 1;
        }
        if normal.y < 0.0 {
            signbits |= 2;
        }
        if normal.z < 0.0 {
            signbits |= 4;
        }
        planes.push(Plane {
            normal,
            dist,
            plane_type,
            signbits,
        });
    }
    Ok(planes)
}

fn parse_clipnodes(data: &[u8]) -> Result<Vec<ClipNode>, BspError> {
    if !data.len().is_multiple_of(DCLIPNODE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut clipnodes = Vec::with_capacity(data.len() / DCLIPNODE_SIZE);
    for chunk in data.chunks_exact(DCLIPNODE_SIZE) {
        let planenum = i32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let child0 = i16::from_le_bytes(chunk[4..6].try_into().unwrap()) as i32;
        let child1 = i16::from_le_bytes(chunk[6..8].try_into().unwrap()) as i32;
        clipnodes.push(ClipNode {
            planenum,
            children: [child0, child1],
        });
    }
    Ok(clipnodes)
}

fn parse_nodes(data: &[u8]) -> Result<Vec<BspNode>, BspError> {
    if !data.len().is_multiple_of(DNODE_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut nodes = Vec::with_capacity(data.len() / DNODE_SIZE);
    for chunk in data.chunks_exact(DNODE_SIZE) {
        let planenum = i32::from_le_bytes(chunk[0..4].try_into().unwrap());
        let child0 = i16::from_le_bytes(chunk[4..6].try_into().unwrap()) as i32;
        let child1 = i16::from_le_bytes(chunk[6..8].try_into().unwrap()) as i32;
        nodes.push(BspNode {
            planenum,
            children: [child0, child1],
        });
    }
    Ok(nodes)
}

fn parse_leaf_contents(data: &[u8]) -> Result<Vec<i32>, BspError> {
    if !data.len().is_multiple_of(DLEAF_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut contents = Vec::with_capacity(data.len() / DLEAF_SIZE);
    for chunk in data.chunks_exact(DLEAF_SIZE) {
        let value = i32::from_le_bytes(chunk[0..4].try_into().unwrap());
        contents.push(value);
    }
    Ok(contents)
}

fn parse_models(data: &[u8]) -> Result<Vec<BspModel>, BspError> {
    if !data.len().is_multiple_of(DMODEL_SIZE) {
        return Err(BspError::InvalidLump);
    }
    let mut models = Vec::with_capacity(data.len() / DMODEL_SIZE);
    for chunk in data.chunks_exact(DMODEL_SIZE) {
        let mins = Vec3::new(
            f32::from_le_bytes(chunk[0..4].try_into().unwrap()),
            f32::from_le_bytes(chunk[4..8].try_into().unwrap()),
            f32::from_le_bytes(chunk[8..12].try_into().unwrap()),
        );
        let maxs = Vec3::new(
            f32::from_le_bytes(chunk[12..16].try_into().unwrap()),
            f32::from_le_bytes(chunk[16..20].try_into().unwrap()),
            f32::from_le_bytes(chunk[20..24].try_into().unwrap()),
        );
        let origin = Vec3::new(
            f32::from_le_bytes(chunk[24..28].try_into().unwrap()),
            f32::from_le_bytes(chunk[28..32].try_into().unwrap()),
            f32::from_le_bytes(chunk[32..36].try_into().unwrap()),
        );
        let mut headnode = [0i32; MAX_MAP_HULLS];
        let mut offset = 36;
        for node in &mut headnode {
            *node = i32::from_le_bytes(chunk[offset..offset + 4].try_into().unwrap());
            offset += 4;
        }
        let visleafs = i32::from_le_bytes(chunk[offset..offset + 4].try_into().unwrap());
        let firstface = i32::from_le_bytes(chunk[offset + 4..offset + 8].try_into().unwrap());
        let numfaces = i32::from_le_bytes(chunk[offset + 8..offset + 12].try_into().unwrap());

        models.push(BspModel {
            mins,
            maxs,
            origin,
            headnode,
            visleafs,
            firstface,
            numfaces,
        });
    }
    Ok(models)
}

fn build_hull0_clipnodes(
    nodes: &[BspNode],
    leaf_contents: &[i32],
) -> Result<Vec<ClipNode>, BspError> {
    let mut clipnodes = Vec::with_capacity(nodes.len());
    for node in nodes {
        let mut children = [0i32; 2];
        for (i, out_child) in children.iter_mut().enumerate() {
            let child = node.children[i];
            *out_child = if child < 0 {
                let leaf_index = (-child - 1) as usize;
                leaf_contents
                    .get(leaf_index)
                    .copied()
                    .ok_or(BspError::InvalidLump)?
            } else {
                child
            };
        }
        clipnodes.push(ClipNode {
            planenum: node.planenum,
            children,
        });
    }
    Ok(clipnodes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::defs::{CONTENTS_EMPTY, CONTENTS_SOLID};

    fn push_f32(data: &mut Vec<u8>, value: f32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i32(data: &mut Vec<u8>, value: i32) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_i16(data: &mut Vec<u8>, value: i16) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u16(data: &mut Vec<u8>, value: u16) {
        data.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u8(data: &mut Vec<u8>, value: u8) {
        data.push(value);
    }

    fn build_bsp(lumps: Vec<Vec<u8>>) -> Vec<u8> {
        let header_size = 4 + HEADER_LUMPS * 8;
        let mut data = Vec::new();
        data.extend_from_slice(&BSP_VERSION.to_le_bytes());

        let mut offset = header_size as u32;
        for i in 0..HEADER_LUMPS {
            let length = lumps[i].len() as u32;
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
    fn parses_header() {
        let mut data = Vec::new();
        data.extend_from_slice(&BSP_VERSION.to_le_bytes());
        for _ in 0..HEADER_LUMPS {
            data.extend_from_slice(&0u32.to_le_bytes());
            data.extend_from_slice(&0u32.to_le_bytes());
        }

        let bsp = Bsp::from_bytes(data).unwrap();
        assert_eq!(bsp.version, BSP_VERSION);
        assert_eq!(bsp.lumps.len(), HEADER_LUMPS);
    }

    #[test]
    fn reads_entities_text() {
        let entities = b"{\"classname\" \"worldspawn\"}\n\0";
        let mut data = Vec::new();
        data.extend_from_slice(&BSP_VERSION.to_le_bytes());

        let header_size = 4 + HEADER_LUMPS * 8;
        let entities_offset = header_size as u32;
        let entities_len = entities.len() as u32;

        for i in 0..HEADER_LUMPS {
            if i == LUMP_ENTITIES {
                data.extend_from_slice(&entities_offset.to_le_bytes());
                data.extend_from_slice(&entities_len.to_le_bytes());
            } else {
                data.extend_from_slice(&0u32.to_le_bytes());
                data.extend_from_slice(&0u32.to_le_bytes());
            }
        }

        data.extend_from_slice(entities);

        let bsp = Bsp::from_bytes(data).unwrap();
        let text = bsp.entities_text().unwrap();
        assert_eq!(text, "{\"classname\" \"worldspawn\"}\n");
    }

    #[test]
    fn computes_map_checksums() {
        let mut data = Vec::new();
        data.extend_from_slice(&BSP_VERSION.to_le_bytes());

        let header_size = 4 + HEADER_LUMPS * 8;
        let mut offsets = Vec::with_capacity(HEADER_LUMPS);
        let mut payloads = Vec::with_capacity(HEADER_LUMPS);
        let mut cursor = header_size;

        for i in 0..HEADER_LUMPS {
            let payload = vec![i as u8; 4];
            offsets.push((cursor as u32, payload.len() as u32));
            cursor += payload.len();
            payloads.push(payload);
        }

        for (offset, length) in &offsets {
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
        }

        for payload in &payloads {
            data.extend_from_slice(payload);
        }

        let bsp = Bsp::from_bytes(data).unwrap();
        let (checksum, checksum2) = bsp.map_checksums().unwrap();

        let mut expected = 0u32;
        let mut expected2 = 0u32;
        for i in 0..HEADER_LUMPS {
            if i == LUMP_ENTITIES {
                continue;
            }
            let value = block_checksum(&payloads[i]);
            expected ^= value;
            if i == LUMP_VISIBILITY || i == LUMP_LEAFS || i == LUMP_NODES {
                continue;
            }
            expected2 ^= value;
        }

        assert_eq!(checksum, expected);
        assert_eq!(checksum2, expected2);
    }

    #[test]
    fn parses_collision_lumps() {
        let mut lumps = vec![Vec::new(); HEADER_LUMPS];

        let mut planes = Vec::new();
        push_f32(&mut planes, 1.0);
        push_f32(&mut planes, 0.0);
        push_f32(&mut planes, 0.0);
        push_f32(&mut planes, 64.0);
        push_i32(&mut planes, 0);
        lumps[LUMP_PLANES] = planes;

        let mut clipnodes = Vec::new();
        push_i32(&mut clipnodes, 0);
        push_i16(&mut clipnodes, CONTENTS_SOLID as i16);
        push_i16(&mut clipnodes, CONTENTS_EMPTY as i16);
        lumps[LUMP_CLIPNODES] = clipnodes;

        let mut nodes = Vec::new();
        push_i32(&mut nodes, 0);
        push_i16(&mut nodes, -1);
        push_i16(&mut nodes, -2);
        for _ in 0..6 {
            push_i16(&mut nodes, 0);
        }
        push_u16(&mut nodes, 0);
        push_u16(&mut nodes, 0);
        lumps[LUMP_NODES] = nodes;

        let mut leafs = Vec::new();
        push_i32(&mut leafs, CONTENTS_SOLID);
        push_i32(&mut leafs, 0);
        for _ in 0..6 {
            push_i16(&mut leafs, 0);
        }
        push_u16(&mut leafs, 0);
        push_u16(&mut leafs, 0);
        for _ in 0..4 {
            push_u8(&mut leafs, 0);
        }

        push_i32(&mut leafs, CONTENTS_EMPTY);
        push_i32(&mut leafs, 0);
        for _ in 0..6 {
            push_i16(&mut leafs, 0);
        }
        push_u16(&mut leafs, 0);
        push_u16(&mut leafs, 0);
        for _ in 0..4 {
            push_u8(&mut leafs, 0);
        }
        lumps[LUMP_LEAFS] = leafs;

        let mut models = Vec::new();
        for _ in 0..9 {
            push_f32(&mut models, 0.0);
        }
        for _ in 0..MAX_MAP_HULLS {
            push_i32(&mut models, 0);
        }
        push_i32(&mut models, 0);
        push_i32(&mut models, 0);
        push_i32(&mut models, 0);
        lumps[LUMP_MODELS] = models;

        let data = build_bsp(lumps);
        let bsp = Bsp::from_bytes(data).unwrap();
        let collision = BspCollision::from_bsp(&bsp).unwrap();

        assert_eq!(collision.planes.len(), 1);
        assert_eq!(collision.clipnodes.len(), 1);
        assert_eq!(collision.hull0_clipnodes.len(), 1);
        assert_eq!(collision.models.len(), 1);

        let hull0 = collision.hull(0, 0).unwrap();
        assert_eq!(
            hull0.clipnodes[0].children,
            [CONTENTS_SOLID, CONTENTS_EMPTY]
        );
        let hull1 = collision.hull(0, 1).unwrap();
        assert_eq!(hull1.clip_mins, HULL1_MINS);
        assert_eq!(hull1.clip_maxs, HULL1_MAXS);
    }
}
