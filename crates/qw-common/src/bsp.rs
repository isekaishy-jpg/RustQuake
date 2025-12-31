// BSP header parsing for QuakeWorld maps (version 29).

use std::fmt;

use crate::block_checksum;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
