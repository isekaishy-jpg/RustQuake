// Quake sprite (SPR) parsing.

use crate::palette::Palette;

const IDSPRITE: u32 = 0x50534449;
const SPR_VERSION: i32 = 1;

#[derive(Debug)]
pub enum SprError {
    InvalidHeader,
    UnsupportedVersion(i32),
    UnexpectedEof,
    UnsupportedFrameType(i32),
    InvalidData(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpriteHeader {
    pub sprite_type: i32,
    pub bounding_radius: f32,
    pub width: u32,
    pub height: u32,
    pub num_frames: u32,
    pub beam_length: f32,
    pub sync_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pub header: SpriteHeader,
    pub frames: Vec<SpriteFrame>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpriteImage {
    pub width: u32,
    pub height: u32,
    pub origin: (i32, i32),
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpriteFrame {
    Single(SpriteImage),
    Group {
        intervals: Vec<f32>,
        frames: Vec<SpriteImage>,
    },
}

impl Sprite {
    pub fn from_bytes(data: &[u8]) -> Result<Self, SprError> {
        let mut cursor = Cursor::new(data);
        let ident = cursor.read_u32()?;
        if ident != IDSPRITE {
            return Err(SprError::InvalidHeader);
        }
        let version = cursor.read_i32()?;
        if version != SPR_VERSION {
            return Err(SprError::UnsupportedVersion(version));
        }

        let sprite_type = cursor.read_i32()?;
        let bounding_radius = cursor.read_f32()?;
        let width = read_count(&mut cursor, "width")?;
        let height = read_count(&mut cursor, "height")?;
        let num_frames = read_count(&mut cursor, "num_frames")?;
        let beam_length = cursor.read_f32()?;
        let sync_type = cursor.read_i32()?;

        let header = SpriteHeader {
            sprite_type,
            bounding_radius,
            width,
            height,
            num_frames,
            beam_length,
            sync_type,
        };

        let mut frames = Vec::with_capacity(num_frames as usize);
        for _ in 0..num_frames {
            let frame_type = cursor.read_i32()?;
            match frame_type {
                0 => {
                    let frame = parse_frame(&mut cursor)?;
                    frames.push(SpriteFrame::Single(frame));
                }
                1 => {
                    let group_count = read_count(&mut cursor, "frame_group_count")?;
                    let mut intervals = Vec::with_capacity(group_count as usize);
                    for _ in 0..group_count {
                        intervals.push(cursor.read_f32()?);
                    }
                    let mut group_frames = Vec::with_capacity(group_count as usize);
                    for _ in 0..group_count {
                        group_frames.push(parse_frame(&mut cursor)?);
                    }
                    frames.push(SpriteFrame::Group {
                        intervals,
                        frames: group_frames,
                    });
                }
                other => return Err(SprError::UnsupportedFrameType(other)),
            }
        }

        Ok(Sprite { header, frames })
    }
}

impl SpriteImage {
    pub fn expand_rgba(&self, palette: &Palette) -> Vec<u8> {
        palette.expand_indices(&self.pixels, Some(255))
    }
}

fn parse_frame(cursor: &mut Cursor<'_>) -> Result<SpriteImage, SprError> {
    let width = read_count(cursor, "frame_width")?;
    let height = read_count(cursor, "frame_height")?;
    let origin_x = cursor.read_i32()?;
    let origin_y = cursor.read_i32()?;
    let size = (width as usize)
        .checked_mul(height as usize)
        .ok_or(SprError::InvalidData("frame size overflow"))?;
    let pixels = cursor.read_bytes(size)?.to_vec();
    Ok(SpriteImage {
        width,
        height,
        origin: (origin_x, origin_y),
        pixels,
    })
}

fn read_count(cursor: &mut Cursor<'_>, label: &'static str) -> Result<u32, SprError> {
    let value = cursor.read_i32()?;
    if value < 0 {
        return Err(SprError::InvalidData(label));
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

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], SprError> {
        if self.offset + len > self.data.len() {
            return Err(SprError::UnexpectedEof);
        }
        let out = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(out)
    }

    fn read_i32(&mut self) -> Result<i32, SprError> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u32(&mut self) -> Result<u32, SprError> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_f32(&mut self) -> Result<f32, SprError> {
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

    fn build_minimal_sprite() -> Vec<u8> {
        let mut data = Vec::new();
        push_u32(&mut data, IDSPRITE);
        push_i32(&mut data, SPR_VERSION);
        push_i32(&mut data, 0);
        push_f32(&mut data, 0.0);
        push_i32(&mut data, 2);
        push_i32(&mut data, 1);
        push_i32(&mut data, 1);
        push_f32(&mut data, 0.0);
        push_i32(&mut data, 0);

        push_i32(&mut data, 0);
        push_i32(&mut data, 2);
        push_i32(&mut data, 1);
        push_i32(&mut data, 0);
        push_i32(&mut data, 0);
        data.extend_from_slice(&[0u8, 1u8]);

        data
    }

    #[test]
    fn parses_minimal_sprite() {
        let bytes = build_minimal_sprite();
        let sprite = Sprite::from_bytes(&bytes).unwrap();
        assert_eq!(sprite.header.num_frames, 1);
        match &sprite.frames[0] {
            SpriteFrame::Single(frame) => {
                assert_eq!(frame.width, 2);
                assert_eq!(frame.height, 1);
                assert_eq!(frame.pixels, vec![0, 1]);
            }
            _ => panic!("expected single frame"),
        }
    }

    #[test]
    fn expands_sprite_frame_to_rgba() {
        let mut palette_bytes = vec![0u8; 256 * 3];
        palette_bytes[0] = 5;
        palette_bytes[1] = 6;
        palette_bytes[2] = 7;
        let palette = Palette::from_bytes(&palette_bytes).unwrap();
        let image = SpriteImage {
            width: 1,
            height: 1,
            origin: (0, 0),
            pixels: vec![0],
        };
        let rgba = image.expand_rgba(&palette);
        assert_eq!(rgba, vec![5, 6, 7, 255]);
    }
}
