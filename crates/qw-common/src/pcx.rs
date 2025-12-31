// PCX image decoding for Quake assets.

use crate::palette::{Palette, PaletteError};

#[derive(Debug)]
pub enum PcxError {
    InvalidHeader,
    UnsupportedFormat,
    UnexpectedEof,
    MissingPalette,
    Palette(PaletteError),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PcxImage {
    pub width: u32,
    pub height: u32,
    pub indices: Vec<u8>,
    pub palette: Option<Palette>,
}

impl PcxImage {
    pub fn from_bytes(data: &[u8]) -> Result<Self, PcxError> {
        if data.len() < 128 {
            return Err(PcxError::InvalidHeader);
        }
        if data[0] != 0x0a || data[2] != 1 {
            return Err(PcxError::InvalidHeader);
        }
        let bits_per_pixel = data[3];
        let x_min = u16::from_le_bytes([data[4], data[5]]);
        let y_min = u16::from_le_bytes([data[6], data[7]]);
        let x_max = u16::from_le_bytes([data[8], data[9]]);
        let y_max = u16::from_le_bytes([data[10], data[11]]);
        let planes = data[65];
        let bytes_per_line = u16::from_le_bytes([data[66], data[67]]) as usize;

        if bits_per_pixel != 8 || planes != 1 {
            return Err(PcxError::UnsupportedFormat);
        }
        if x_max < x_min || y_max < y_min {
            return Err(PcxError::InvalidHeader);
        }

        let width = (x_max - x_min + 1) as usize;
        let height = (y_max - y_min + 1) as usize;
        if bytes_per_line < width {
            return Err(PcxError::InvalidHeader);
        }

        let mut indices = Vec::with_capacity(width * height);
        let mut offset = 128;
        for _ in 0..height {
            let mut row = Vec::with_capacity(bytes_per_line);
            while row.len() < bytes_per_line {
                let byte = *data.get(offset).ok_or(PcxError::UnexpectedEof)?;
                offset += 1;
                if byte & 0xc0 == 0xc0 {
                    let count = (byte & 0x3f) as usize;
                    let value = *data.get(offset).ok_or(PcxError::UnexpectedEof)?;
                    offset += 1;
                    row.extend(std::iter::repeat(value).take(count));
                } else {
                    row.push(byte);
                }
            }
            indices.extend_from_slice(&row[..width]);
        }

        let palette = if data.len() >= 769 {
            let palette_start = data.len() - 769;
            if data[palette_start] == 0x0c {
                Some(Palette::from_bytes(&data[palette_start + 1..])?)
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            width: width as u32,
            height: height as u32,
            indices,
            palette,
        })
    }

    pub fn expand_rgba(&self, fallback: Option<&Palette>) -> Result<Vec<u8>, PcxError> {
        let palette = self
            .palette
            .as_ref()
            .or(fallback)
            .ok_or(PcxError::MissingPalette)?;
        Ok(palette.expand_indices(&self.indices, None))
    }
}

impl From<PaletteError> for PcxError {
    fn from(err: PaletteError) -> Self {
        PcxError::Palette(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_pcx_2x2() -> Vec<u8> {
        let mut data = vec![0u8; 128];
        data[0] = 0x0a;
        data[1] = 5;
        data[2] = 1;
        data[3] = 8;
        data[8..10].copy_from_slice(&1u16.to_le_bytes());
        data[10..12].copy_from_slice(&1u16.to_le_bytes());
        data[65] = 1;
        data[66..68].copy_from_slice(&2u16.to_le_bytes());

        data.extend_from_slice(&[0u8, 1u8, 2u8, 3u8]);

        data.push(0x0c);
        let mut palette = vec![0u8; 768];
        palette[0..3].copy_from_slice(&[10, 20, 30]);
        palette[3..6].copy_from_slice(&[40, 50, 60]);
        palette[6..9].copy_from_slice(&[70, 80, 90]);
        palette[9..12].copy_from_slice(&[100, 110, 120]);
        data.extend_from_slice(&palette);

        data
    }

    #[test]
    fn decodes_indices_and_palette() {
        let bytes = build_pcx_2x2();
        let pcx = PcxImage::from_bytes(&bytes).unwrap();
        assert_eq!(pcx.width, 2);
        assert_eq!(pcx.height, 2);
        assert_eq!(pcx.indices, vec![0, 1, 2, 3]);
        assert!(pcx.palette.is_some());
        let rgba = pcx.expand_rgba(None).unwrap();
        assert_eq!(rgba[0..4], [10, 20, 30, 255]);
        assert_eq!(rgba[4..8], [40, 50, 60, 255]);
    }
}
