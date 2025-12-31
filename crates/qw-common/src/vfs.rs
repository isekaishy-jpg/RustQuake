// Quake-style search paths over directories and PAK files.

use crate::pak::{Pak, PakError};
use std::fs;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub enum FsError {
    InvalidDir,
    InvalidPath,
    NotFound,
    Pak(PakError),
    Io(std::io::Error),
}

impl From<PakError> for FsError {
    fn from(err: PakError) -> Self {
        FsError::Pak(err)
    }
}

impl From<std::io::Error> for FsError {
    fn from(err: std::io::Error) -> Self {
        FsError::Io(err)
    }
}

#[derive(Debug)]
enum SearchPath {
    Pack(Pak),
    Dir(PathBuf),
}

#[derive(Debug, Default)]
pub struct QuakeFs {
    search_paths: Vec<SearchPath>,
}

impl QuakeFs {
    pub fn new() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.search_paths.is_empty()
    }

    pub fn add_game_dir(&mut self, dir: impl AsRef<Path>) -> Result<(), FsError> {
        let dir = dir.as_ref().to_path_buf();
        if !dir.is_dir() {
            return Err(FsError::InvalidDir);
        }

        let mut packs = Vec::new();
        for index in 0.. {
            let pak_path = dir.join(format!("pak{}.pak", index));
            if !pak_path.exists() {
                break;
            }
            packs.push(Pak::open(pak_path)?);
        }

        let mut new_paths = Vec::new();
        for pack in packs.into_iter().rev() {
            new_paths.push(SearchPath::Pack(pack));
        }
        new_paths.push(SearchPath::Dir(dir));

        self.search_paths.splice(0..0, new_paths);
        Ok(())
    }

    pub fn contains(&self, name: &str) -> bool {
        if !is_safe_relative_path(name) {
            return false;
        }

        for search in &self.search_paths {
            match search {
                SearchPath::Pack(pack) => {
                    if pack.find(name).is_some() {
                        return true;
                    }
                }
                SearchPath::Dir(dir) => {
                    if dir.join(name).is_file() {
                        return true;
                    }
                }
            }
        }

        false
    }

    pub fn read(&self, name: &str) -> Result<Vec<u8>, FsError> {
        if !is_safe_relative_path(name) {
            return Err(FsError::InvalidPath);
        }

        for search in &self.search_paths {
            match search {
                SearchPath::Pack(pack) => {
                    if let Some(entry) = pack.find(name) {
                        return Ok(pack.read(entry)?);
                    }
                }
                SearchPath::Dir(dir) => {
                    let path = dir.join(name);
                    if path.is_file() {
                        return Ok(fs::read(path)?);
                    }
                }
            }
        }

        Err(FsError::NotFound)
    }
}

fn is_safe_relative_path(name: &str) -> bool {
    if name.is_empty() || name.contains(':') || name.contains('\0') {
        return false;
    }

    let path = Path::new(name);
    if path.is_absolute() {
        return false;
    }

    for component in path.components() {
        match component {
            Component::ParentDir | Component::Prefix(_) | Component::RootDir => {
                return false;
            }
            _ => {}
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = std::env::temp_dir();
        path.push(format!("rustquake-test-{}-{}", process::id(), nanos));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn write_pak(path: &Path, entries: &[(&str, &[u8])]) -> std::io::Result<()> {
        let mut data = Vec::new();
        data.resize(12, 0);

        let mut offsets = Vec::new();
        for (_, bytes) in entries {
            let offset = data.len() as u32;
            data.extend_from_slice(bytes);
            offsets.push((offset, bytes.len() as u32));
        }

        let dir_offset = data.len() as u32;
        let dir_len = (entries.len() * 64) as u32;

        for ((name, _), (offset, length)) in entries.iter().zip(offsets.iter()) {
            let mut name_buf = [0u8; 56];
            let name_bytes = name.as_bytes();
            let copy_len = name_bytes.len().min(55);
            name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            data.extend_from_slice(&name_buf);
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
        }

        data[0..4].copy_from_slice(b"PACK");
        data[4..8].copy_from_slice(&dir_offset.to_le_bytes());
        data[8..12].copy_from_slice(&dir_len.to_le_bytes());

        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    #[test]
    fn prefers_latest_pak_over_directory() {
        let dir = temp_dir();
        fs::write(dir.join("foo.txt"), b"dir").unwrap();

        write_pak(dir.join("pak0.pak").as_path(), &[("foo.txt", b"pak0")]).unwrap();
        write_pak(dir.join("pak1.pak").as_path(), &[("foo.txt", b"pak1")]).unwrap();

        let mut fsys = QuakeFs::new();
        fsys.add_game_dir(&dir).unwrap();

        let data = fsys.read("foo.txt").unwrap();
        assert_eq!(data, b"pak1");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_unsafe_paths() {
        let fsys = QuakeFs::new();
        assert!(!fsys.contains("../id1/pak0.pak"));
        assert!(!fsys.contains("C:\\id1\\pak0.pak"));
    }
}
