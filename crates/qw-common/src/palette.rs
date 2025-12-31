// Quake palette loading and expansion helpers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug)]
pub enum PaletteError {
    InvalidLength,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    colors: [Rgb; 256],
}

impl Palette {
    pub fn from_bytes(data: &[u8]) -> Result<Self, PaletteError> {
        if data.len() < 256 * 3 {
            return Err(PaletteError::InvalidLength);
        }
        let mut colors = [Rgb(0, 0, 0); 256];
        for (i, color) in colors.iter_mut().enumerate() {
            let base = i * 3;
            *color = Rgb(data[base], data[base + 1], data[base + 2]);
        }
        Ok(Self { colors })
    }

    pub fn rgba_for(&self, index: u8, transparent_index: Option<u8>) -> [u8; 4] {
        if transparent_index == Some(index) {
            return [0, 0, 0, 0];
        }
        let Rgb(r, g, b) = self.colors[index as usize];
        [r, g, b, 255]
    }

    pub fn expand_indices(&self, indices: &[u8], transparent_index: Option<u8>) -> Vec<u8> {
        let mut out = Vec::with_capacity(indices.len() * 4);
        for &idx in indices {
            out.extend_from_slice(&self.rgba_for(idx, transparent_index));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_palette_bytes() {
        let mut bytes = Vec::new();
        for i in 0..256 {
            bytes.push(i as u8);
            bytes.push(0);
            bytes.push(255u8.saturating_sub(i as u8));
        }

        let palette = Palette::from_bytes(&bytes).unwrap();
        assert_eq!(palette.rgba_for(0, None), [0, 0, 255, 255]);
        assert_eq!(palette.rgba_for(255, None), [255, 0, 0, 255]);
        assert_eq!(palette.rgba_for(7, Some(7)), [0, 0, 0, 0]);
    }

    #[test]
    fn expands_indices_to_rgba() {
        let bytes = [10u8, 20, 30].repeat(256);
        let palette = Palette::from_bytes(&bytes).unwrap();
        let rgba = palette.expand_indices(&[0, 1], None);
        assert_eq!(rgba, vec![10, 20, 30, 255, 10, 20, 30, 255]);
    }
}
