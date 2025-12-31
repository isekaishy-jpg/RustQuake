// WAD2 reader for Quake lumps.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum WadError {
    Io(std::io::Error),
    InvalidHeader,
    InvalidDirectory,
    UnsupportedCompression(u8),
    EntryNotFound,
}

impl From<std::io::Error> for WadError {
    fn from(err: std::io::Error) -> Self {
        WadError::Io(err)
    }
}

#[derive(Debug, Clone)]
pub struct WadEntry {
    pub name: String,
    pub filepos: u32,
    pub disksize: u32,
    pub size: u32,
    pub typ: u8,
    pub compression: u8,
}

#[derive(Debug)]
pub struct Wad {
    path: PathBuf,
    data: Vec<u8>,
    entries: Vec<WadEntry>,
}

impl Wad {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, WadError> {
        let path = path.as_ref().to_path_buf();
        let data = fs::read(&path)?;
        Self::parse_with_path(path, data)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, WadError> {
        Self::parse_with_path(PathBuf::from("<memory>"), data)
    }

    pub fn entries(&self) -> &[WadEntry] {
        &self.entries
    }

    pub fn find(&self, name: &str) -> Option<&WadEntry> {
        let cleaned = cleanup_name_str(name);
        self.entries.iter().find(|entry| entry.name == cleaned)
    }

    pub fn get(&self, name: &str) -> Result<&[u8], WadError> {
        let entry = self.find(name).ok_or(WadError::EntryNotFound)?;
        self.get_entry(entry)
    }

    pub fn get_entry(&self, entry: &WadEntry) -> Result<&[u8], WadError> {
        if entry.compression != 0 {
            return Err(WadError::UnsupportedCompression(entry.compression));
        }
        let start = entry.filepos as usize;
        let end = start + entry.disksize as usize;
        if end > self.data.len() {
            return Err(WadError::InvalidDirectory);
        }
        Ok(&self.data[start..end])
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn parse_with_path(path: PathBuf, data: Vec<u8>) -> Result<Self, WadError> {
        if data.len() < 12 {
            return Err(WadError::InvalidHeader);
        }

        if &data[0..4] != b"WAD2" {
            return Err(WadError::InvalidHeader);
        }

        let num_lumps = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
        let dir_offset = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
        let dir_len = num_lumps * 32;

        if dir_offset + dir_len > data.len() {
            return Err(WadError::InvalidDirectory);
        }

        let mut entries = Vec::with_capacity(num_lumps);
        let mut offset = dir_offset;
        for _ in 0..num_lumps {
            let filepos = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
            let disksize = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap());
            let size = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().unwrap());
            let typ = data[offset + 12];
            let compression = data[offset + 13];
            let name_bytes = &data[offset + 16..offset + 32];
            let name = cleanup_name_bytes(name_bytes);
            entries.push(WadEntry {
                name,
                filepos,
                disksize,
                size,
                typ,
                compression,
            });

            offset += 32;
        }

        Ok(Wad {
            path,
            data,
            entries,
        })
    }
}

fn cleanup_name_str(name: &str) -> String {
    cleanup_name_bytes(name.as_bytes())
}

fn cleanup_name_bytes(bytes: &[u8]) -> String {
    let mut out = [0u8; 16];
    let max_len = bytes.len().min(16);
    for i in 0..max_len {
        let mut c = bytes[i];
        if c.is_ascii_uppercase() {
            c = c.to_ascii_lowercase();
        }
        out[i] = c;
    }
    let trimmed_len = out.iter().position(|b| *b == 0).unwrap_or(16);
    String::from_utf8_lossy(&out[..trimmed_len]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
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

    fn write_wad(path: &Path, entries: &[(&str, &[u8])]) -> std::io::Result<()> {
        let mut data = Vec::new();
        data.resize(12, 0);

        let mut offsets = Vec::new();
        for (_, bytes) in entries {
            let offset = data.len() as u32;
            data.extend_from_slice(bytes);
            offsets.push((offset, bytes.len() as u32));
        }

        let dir_offset = data.len() as u32;
        for ((name, _), (offset, length)) in entries.iter().zip(offsets.iter()) {
            data.extend_from_slice(&offset.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
            data.extend_from_slice(&length.to_le_bytes());
            data.push(0);
            data.push(0);
            data.push(0);
            data.push(0);

            let mut name_buf = [0u8; 16];
            let name_bytes = name.as_bytes();
            let copy_len = name_bytes.len().min(15);
            name_buf[..copy_len].copy_from_slice(&name_bytes[..copy_len]);
            data.extend_from_slice(&name_buf);
        }

        data[0..4].copy_from_slice(b"WAD2");
        data[4..8].copy_from_slice(&(entries.len() as u32).to_le_bytes());
        data[8..12].copy_from_slice(&dir_offset.to_le_bytes());

        let mut file = File::create(path)?;
        file.write_all(&data)?;
        Ok(())
    }

    #[test]
    fn reads_wad_entry() {
        let dir = temp_dir();
        let wad_path = dir.join("gfx.wad");
        write_wad(wad_path.as_path(), &[("CONCHARS", b"abcd")]).unwrap();

        let wad = Wad::open(&wad_path).unwrap();
        let data = wad.get("conchars").unwrap();
        assert_eq!(data, b"abcd");

        let wad = Wad::from_bytes(fs::read(&wad_path).unwrap()).unwrap();
        let data = wad.get("conchars").unwrap();
        assert_eq!(data, b"abcd");

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_invalid_header() {
        let dir = temp_dir();
        let wad_path = dir.join("bad.wad");
        fs::write(&wad_path, b"BAD!").unwrap();

        let err = Wad::open(&wad_path).unwrap_err();
        match err {
            WadError::InvalidHeader => {}
            other => panic!("unexpected error: {:?}", other),
        }

        fs::remove_dir_all(dir).ok();
    }
}
