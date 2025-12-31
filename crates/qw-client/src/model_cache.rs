use std::path::Path;

use qw_common::{AliasModel, FsError, MdlError, Palette, QuakeFs, SprError, Sprite, SpriteFrame};

#[derive(Debug)]
#[allow(dead_code)]
pub enum ModelCacheError {
    Fs(FsError),
    Mdl(MdlError),
    Spr(SprError),
    UnsupportedFormat(String),
}

impl From<FsError> for ModelCacheError {
    fn from(err: FsError) -> Self {
        ModelCacheError::Fs(err)
    }
}

impl From<MdlError> for ModelCacheError {
    fn from(err: MdlError) -> Self {
        ModelCacheError::Mdl(err)
    }
}

impl From<SprError> for ModelCacheError {
    fn from(err: SprError) -> Self {
        ModelCacheError::Spr(err)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelTexture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelKind {
    Alias(AliasModel),
    Sprite(Sprite),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelAsset {
    pub kind: ModelKind,
    pub textures: Vec<ModelTexture>,
}

#[derive(Debug, Default)]
pub struct ModelCache {
    models: Vec<Option<ModelAsset>>,
}

impl ModelCache {
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    pub fn clear(&mut self) {
        self.models.clear();
    }

    #[allow(dead_code)]
    pub fn models(&self) -> &[Option<ModelAsset>] {
        &self.models
    }

    pub fn load(
        &mut self,
        fs: &QuakeFs,
        palette: Option<&Palette>,
        names: &[String],
    ) -> Result<(), ModelCacheError> {
        self.models.clear();
        self.models.resize_with(names.len(), || None);

        for (index, name) in names.iter().enumerate() {
            if name.is_empty() || name.starts_with('*') {
                continue;
            }
            let ext = Path::new(name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase());
            let Some(ext) = ext else {
                continue;
            };

            let bytes = match fs.read(name) {
                Ok(bytes) => bytes,
                Err(FsError::NotFound) => continue,
                Err(err) => return Err(err.into()),
            };

            let asset = match ext.as_str() {
                "mdl" => {
                    let model = AliasModel::from_bytes(&bytes)?;
                    let textures = build_alias_textures(&model, palette);
                    ModelAsset {
                        kind: ModelKind::Alias(model),
                        textures,
                    }
                }
                "spr" => {
                    let sprite = Sprite::from_bytes(&bytes)?;
                    let textures = build_sprite_textures(&sprite, palette);
                    ModelAsset {
                        kind: ModelKind::Sprite(sprite),
                        textures,
                    }
                }
                "bsp" => continue,
                _ => return Err(ModelCacheError::UnsupportedFormat(name.clone())),
            };

            if index < self.models.len() {
                self.models[index] = Some(asset);
            }
        }

        Ok(())
    }
}

fn build_alias_textures(model: &AliasModel, palette: Option<&Palette>) -> Vec<ModelTexture> {
    let Some(palette) = palette else {
        return Vec::new();
    };

    let mut textures = Vec::new();
    for skin in &model.skins {
        match skin {
            qw_common::MdlSkin::Single {
                width,
                height,
                indices,
            } => {
                textures.push(ModelTexture {
                    width: *width,
                    height: *height,
                    rgba: palette.expand_indices(indices, Some(255)),
                });
            }
            qw_common::MdlSkin::Group {
                width,
                height,
                frames,
                ..
            } => {
                for indices in frames {
                    textures.push(ModelTexture {
                        width: *width,
                        height: *height,
                        rgba: palette.expand_indices(indices, Some(255)),
                    });
                }
            }
        }
    }
    textures
}

fn build_sprite_textures(sprite: &Sprite, palette: Option<&Palette>) -> Vec<ModelTexture> {
    let Some(palette) = palette else {
        return Vec::new();
    };

    let mut textures = Vec::new();
    for frame in &sprite.frames {
        match frame {
            SpriteFrame::Single(image) => {
                textures.push(ModelTexture {
                    width: image.width,
                    height: image.height,
                    rgba: image.expand_rgba(palette),
                });
            }
            SpriteFrame::Group { frames, .. } => {
                for image in frames {
                    textures.push(ModelTexture {
                        width: image.width,
                        height: image.height,
                        rgba: image.expand_rgba(palette),
                    });
                }
            }
        }
    }

    textures
}
