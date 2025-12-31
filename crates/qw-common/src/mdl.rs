// Quake alias model (MDL) parsing.

use crate::Vec3;
use crate::palette::Palette;

const IDPOLYHEADER: u32 = 0x4f504449;
const MDL_VERSION: i32 = 6;

#[derive(Debug)]
pub enum MdlError {
    InvalidHeader,
    UnsupportedVersion(i32),
    UnexpectedEof,
    UnsupportedSkinType(i32),
    UnsupportedFrameType(i32),
    InvalidData(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdlHeader {
    pub scale: Vec3,
    pub translate: Vec3,
    pub bounding_radius: f32,
    pub eye_position: Vec3,
    pub num_skins: u32,
    pub skin_width: u32,
    pub skin_height: u32,
    pub num_verts: u32,
    pub num_tris: u32,
    pub num_frames: u32,
    pub sync_type: i32,
    pub flags: i32,
    pub size: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AliasModel {
    pub header: MdlHeader,
    pub skins: Vec<MdlSkin>,
    pub triangles: Vec<MdlTriangle>,
    pub frames: Vec<MdlFrameGroup>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MdlSkin {
    Single {
        width: u32,
        height: u32,
        indices: Vec<u8>,
    },
    Group {
        width: u32,
        height: u32,
        intervals: Vec<f32>,
        frames: Vec<Vec<u8>>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdlTriangle {
    pub faces_front: bool,
    pub indices: [u16; 3],
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdlVertex {
    pub position: Vec3,
    pub normal_index: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdlFrame {
    pub name: String,
    pub vertices: Vec<MdlVertex>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MdlFrameGroup {
    Single(MdlFrame),
    Group {
        intervals: Vec<f32>,
        frames: Vec<MdlFrame>,
    },
}

impl AliasModel {
    pub fn from_bytes(data: &[u8]) -> Result<Self, MdlError> {
        let mut cursor = Cursor::new(data);
        let ident = cursor.read_u32()?;
        if ident != IDPOLYHEADER {
            return Err(MdlError::InvalidHeader);
        }
        let version = cursor.read_i32()?;
        if version != MDL_VERSION {
            return Err(MdlError::UnsupportedVersion(version));
        }

        let scale = Vec3::new(cursor.read_f32()?, cursor.read_f32()?, cursor.read_f32()?);
        let translate = Vec3::new(cursor.read_f32()?, cursor.read_f32()?, cursor.read_f32()?);
        let bounding_radius = cursor.read_f32()?;
        let eye_position = Vec3::new(cursor.read_f32()?, cursor.read_f32()?, cursor.read_f32()?);

        let num_skins = read_count(&mut cursor, "num_skins")?;
        let skin_width = read_count(&mut cursor, "skin_width")?;
        let skin_height = read_count(&mut cursor, "skin_height")?;
        let num_verts = read_count(&mut cursor, "num_verts")?;
        let num_tris = read_count(&mut cursor, "num_tris")?;
        let num_frames = read_count(&mut cursor, "num_frames")?;
        let sync_type = cursor.read_i32()?;
        let flags = cursor.read_i32()?;
        let size = cursor.read_f32()?;

        let header = MdlHeader {
            scale,
            translate,
            bounding_radius,
            eye_position,
            num_skins,
            skin_width,
            skin_height,
            num_verts,
            num_tris,
            num_frames,
            sync_type,
            flags,
            size,
        };

        let skins = parse_skins(&mut cursor, &header)?;
        let triangles = parse_triangles(&mut cursor, header.num_tris)?;
        let frames = parse_frames(&mut cursor, &header)?;

        Ok(AliasModel {
            header,
            skins,
            triangles,
            frames,
        })
    }

    pub fn frame_at_time(&self, index: usize, time: f32) -> Option<&MdlFrame> {
        let group = self.frames.get(index)?;
        group.frame_at_time(time)
    }
}

impl MdlSkin {
    pub fn expand_rgba(&self, palette: &Palette) -> Vec<Vec<u8>> {
        match self {
            MdlSkin::Single { indices, .. } => vec![palette.expand_indices(indices, Some(255))],
            MdlSkin::Group { frames, .. } => frames
                .iter()
                .map(|indices| palette.expand_indices(indices, Some(255)))
                .collect(),
        }
    }
}

impl MdlFrameGroup {
    pub fn frame_at_time(&self, time: f32) -> Option<&MdlFrame> {
        match self {
            MdlFrameGroup::Single(frame) => Some(frame),
            MdlFrameGroup::Group { intervals, frames } => {
                if frames.is_empty() {
                    return None;
                }
                if intervals.is_empty() {
                    return frames.first();
                }

                let total: f32 = intervals.iter().sum();
                if total <= 0.0 {
                    return frames.first();
                }
                let mut t = time % total;
                if t < 0.0 {
                    t += total;
                }
                let count = intervals.len().min(frames.len());
                for idx in 0..count {
                    let interval = intervals[idx];
                    if t < interval {
                        return frames.get(idx);
                    }
                    t -= interval;
                }
                frames.first()
            }
        }
    }
}

fn parse_skins(cursor: &mut Cursor<'_>, header: &MdlHeader) -> Result<Vec<MdlSkin>, MdlError> {
    let skin_size = (header.skin_width as usize)
        .checked_mul(header.skin_height as usize)
        .ok_or(MdlError::InvalidData("skin size overflow"))?;

    let mut skins = Vec::with_capacity(header.num_skins as usize);
    for _ in 0..header.num_skins {
        let skin_type = cursor.read_i32()?;
        match skin_type {
            0 => {
                let indices = cursor.read_bytes(skin_size)?.to_vec();
                skins.push(MdlSkin::Single {
                    width: header.skin_width,
                    height: header.skin_height,
                    indices,
                });
            }
            1 => {
                let group_count = read_count(cursor, "skin_group_count")?;
                let mut intervals = Vec::with_capacity(group_count as usize);
                for _ in 0..group_count {
                    intervals.push(cursor.read_f32()?);
                }
                let mut frames = Vec::with_capacity(group_count as usize);
                for _ in 0..group_count {
                    frames.push(cursor.read_bytes(skin_size)?.to_vec());
                }
                skins.push(MdlSkin::Group {
                    width: header.skin_width,
                    height: header.skin_height,
                    intervals,
                    frames,
                });
            }
            other => return Err(MdlError::UnsupportedSkinType(other)),
        }
    }
    Ok(skins)
}

fn parse_triangles(cursor: &mut Cursor<'_>, count: u32) -> Result<Vec<MdlTriangle>, MdlError> {
    let mut triangles = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let faces_front = cursor.read_i32()? != 0;
        let mut indices = [0u16; 3];
        for slot in &mut indices {
            let value = cursor.read_i32()?;
            if value < 0 || value > u16::MAX as i32 {
                return Err(MdlError::InvalidData("triangle index out of range"));
            }
            *slot = value as u16;
        }
        triangles.push(MdlTriangle {
            faces_front,
            indices,
        });
    }
    Ok(triangles)
}

fn parse_frames(
    cursor: &mut Cursor<'_>,
    header: &MdlHeader,
) -> Result<Vec<MdlFrameGroup>, MdlError> {
    let mut frames = Vec::with_capacity(header.num_frames as usize);
    for _ in 0..header.num_frames {
        let frame_type = cursor.read_i32()?;
        match frame_type {
            0 => {
                let frame = parse_frame(cursor, header)?;
                frames.push(MdlFrameGroup::Single(frame));
            }
            1 => {
                let group_count = read_count(cursor, "frame_group_count")?;
                let mut intervals = Vec::with_capacity(group_count as usize);
                for _ in 0..group_count {
                    intervals.push(cursor.read_f32()?);
                }
                let mut group_frames = Vec::with_capacity(group_count as usize);
                for _ in 0..group_count {
                    group_frames.push(parse_frame(cursor, header)?);
                }
                frames.push(MdlFrameGroup::Group {
                    intervals,
                    frames: group_frames,
                });
            }
            other => return Err(MdlError::UnsupportedFrameType(other)),
        }
    }
    Ok(frames)
}

fn parse_frame(cursor: &mut Cursor<'_>, header: &MdlHeader) -> Result<MdlFrame, MdlError> {
    let _bbox_min = read_trivertx(cursor)?;
    let _bbox_max = read_trivertx(cursor)?;
    let name = read_string(cursor.read_bytes(16)?);

    let mut vertices = Vec::with_capacity(header.num_verts as usize);
    for _ in 0..header.num_verts {
        let (pos, normal_index) = read_trivertx(cursor)?;
        let position = Vec3::new(
            header.scale.x * pos[0] as f32 + header.translate.x,
            header.scale.y * pos[1] as f32 + header.translate.y,
            header.scale.z * pos[2] as f32 + header.translate.z,
        );
        vertices.push(MdlVertex {
            position,
            normal_index,
        });
    }

    Ok(MdlFrame { name, vertices })
}

fn read_trivertx(cursor: &mut Cursor<'_>) -> Result<([u8; 3], u8), MdlError> {
    let x = cursor.read_u8()?;
    let y = cursor.read_u8()?;
    let z = cursor.read_u8()?;
    let normal_index = cursor.read_u8()?;
    Ok(([x, y, z], normal_index))
}

fn read_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

fn read_count(cursor: &mut Cursor<'_>, label: &'static str) -> Result<u32, MdlError> {
    let value = cursor.read_i32()?;
    if value < 0 {
        return Err(MdlError::InvalidData(label));
    }
    Ok(value as u32)
}

struct Cursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], MdlError> {
        if self.offset + len > self.data.len() {
            return Err(MdlError::UnexpectedEof);
        }
        let out = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(out)
    }

    fn read_u8(&mut self) -> Result<u8, MdlError> {
        let bytes = self.read_bytes(1)?;
        Ok(bytes[0])
    }

    fn read_i32(&mut self) -> Result<i32, MdlError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, MdlError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_f32(&mut self) -> Result<f32, MdlError> {
        let bytes = self.read_bytes(4)?;
        Ok(f32::from_le_bytes(bytes.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_i32(buf: &mut Vec<u8>, value: i32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_f32(buf: &mut Vec<u8>, value: f32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_trivertx(buf: &mut Vec<u8>, x: u8, y: u8, z: u8, normal: u8) {
        buf.push(x);
        buf.push(y);
        buf.push(z);
        buf.push(normal);
    }

    fn build_minimal_mdl() -> Vec<u8> {
        let mut data = Vec::new();
        push_u32(&mut data, IDPOLYHEADER);
        push_i32(&mut data, MDL_VERSION);
        push_f32(&mut data, 1.0);
        push_f32(&mut data, 1.0);
        push_f32(&mut data, 1.0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 10.0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);
        push_f32(&mut data, 0.0);
        push_i32(&mut data, 1);
        push_i32(&mut data, 1);
        push_i32(&mut data, 1);
        push_i32(&mut data, 3);
        push_i32(&mut data, 1);
        push_i32(&mut data, 1);
        push_i32(&mut data, 0);
        push_i32(&mut data, 0);
        push_f32(&mut data, 0.0);

        push_i32(&mut data, 0);
        data.push(0);

        push_i32(&mut data, 1);
        push_i32(&mut data, 0);
        push_i32(&mut data, 1);
        push_i32(&mut data, 2);

        push_i32(&mut data, 0);
        push_trivertx(&mut data, 0, 0, 0, 0);
        push_trivertx(&mut data, 0, 0, 0, 0);
        let mut name = [0u8; 16];
        name[..5].copy_from_slice(b"frame");
        data.extend_from_slice(&name);
        push_trivertx(&mut data, 1, 2, 3, 4);
        push_trivertx(&mut data, 4, 5, 6, 7);
        push_trivertx(&mut data, 7, 8, 9, 10);

        data
    }

    #[test]
    fn parses_minimal_alias_model() {
        let bytes = build_minimal_mdl();
        let model = AliasModel::from_bytes(&bytes).unwrap();
        assert_eq!(model.header.num_verts, 3);
        assert_eq!(model.skins.len(), 1);
        assert_eq!(model.triangles.len(), 1);
        match &model.frames[0] {
            MdlFrameGroup::Single(frame) => {
                assert_eq!(frame.name, "frame");
                assert_eq!(frame.vertices[0].position, Vec3::new(1.0, 2.0, 3.0));
                assert_eq!(frame.vertices[1].normal_index, 7);
            }
            _ => panic!("expected single frame"),
        }
    }

    #[test]
    fn expands_skin_to_rgba() {
        let mut palette_bytes = vec![0u8; 256 * 3];
        palette_bytes[0] = 10;
        palette_bytes[1] = 20;
        palette_bytes[2] = 30;
        let palette = Palette::from_bytes(&palette_bytes).unwrap();
        let skin = MdlSkin::Single {
            width: 1,
            height: 1,
            indices: vec![0],
        };
        let rgba = skin.expand_rgba(&palette);
        assert_eq!(rgba[0], vec![10, 20, 30, 255]);
    }

    #[test]
    fn selects_group_frame_by_time() {
        let frame_a = MdlFrame {
            name: "a".to_string(),
            vertices: Vec::new(),
        };
        let frame_b = MdlFrame {
            name: "b".to_string(),
            vertices: Vec::new(),
        };
        let group = MdlFrameGroup::Group {
            intervals: vec![0.1, 0.2],
            frames: vec![frame_a, frame_b],
        };

        assert_eq!(group.frame_at_time(0.05).unwrap().name, "a");
        assert_eq!(group.frame_at_time(0.15).unwrap().name, "b");
        assert_eq!(group.frame_at_time(0.35).unwrap().name, "a");
    }
}
