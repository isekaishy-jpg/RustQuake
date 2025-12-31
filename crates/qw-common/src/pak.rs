// PAK file reader (id Tech 1 format).

use crate::crc::crc_block;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PakError {
    Io(std::io::Error),
    InvalidHeader,
    InvalidDirectory,
    EntryNotFound,
}

impl From<std::io::Error> for PakError {
    fn from(err: std::io::Error) -> Self {
        PakError::Io(err)
    }
}

#[derive(Debug, Clone)]
pub struct PakEntry {
    pub name: String,
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug)]
pub struct Pak {
    path: PathBuf,
    entries: Vec<PakEntry>,
    dir_crc: u16,
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
    fn reads_entry() {
        let dir = temp_dir();
        let pak_path = dir.join("pak0.pak");
        write_pak(pak_path.as_path(), &[("test.txt", b"hello")]).unwrap();

        let pak = Pak::open(pak_path).unwrap();
        let data = pak.read_by_name("test.txt").unwrap();
        assert_eq!(data, b"hello");
        assert_ne!(pak.dir_crc(), 0);

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rejects_invalid_header() {
        let dir = temp_dir();
        let pak_path = dir.join("pak0.pak");
        fs::write(&pak_path, b"BAD!00000000").unwrap();

        let err = Pak::open(&pak_path).unwrap_err();
        match err {
            PakError::InvalidHeader => {}
            other => panic!("unexpected error: {:?}", other),
        }

        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn finds_entries_case_insensitively() {
        let dir = temp_dir();
        let pak_path = dir.join("pak0.pak");
        write_pak(pak_path.as_path(), &[("SOUND/TEST.WAV", b"data")]).unwrap();

        let pak = Pak::open(pak_path).unwrap();
        let entry = pak.find_case_insensitive("sound/test.wav");
        assert!(entry.is_some());

        fs::remove_dir_all(dir).ok();
    }
}

impl Pak {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, PakError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;

        let mut header = [0u8; 12];
        file.read_exact(&mut header)?;
        if &header[0..4] != b"PACK" {
            return Err(PakError::InvalidHeader);
        }

        let dir_offset = u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64;
        let dir_length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as u64;
        if dir_length % 64 != 0 {
            return Err(PakError::InvalidDirectory);
        }

        let entry_count = (dir_length / 64) as usize;
        file.seek(SeekFrom::Start(dir_offset))?;
        let mut dir_data = vec![0u8; dir_length as usize];
        file.read_exact(&mut dir_data)?;

        let dir_crc = crc_block(&dir_data);

        let mut entries = Vec::with_capacity(entry_count);
        let mut cursor = 0usize;
        for _ in 0..entry_count {
            let name_buf: [u8; 56] = dir_data[cursor..cursor + 56].try_into().unwrap();
            let name_len = name_buf.iter().position(|b| *b == 0).unwrap_or(56);
            let name = String::from_utf8_lossy(&name_buf[..name_len]).to_string();

            let offset = u32::from_le_bytes(dir_data[cursor + 56..cursor + 60].try_into().unwrap());
            let length = u32::from_le_bytes(dir_data[cursor + 60..cursor + 64].try_into().unwrap());
            entries.push(PakEntry {
                name,
                offset,
                length,
            });
            cursor += 64;
        }

        Ok(Pak {
            path,
            entries,
            dir_crc,
        })
    }

    pub fn entries(&self) -> &[PakEntry] {
        &self.entries
    }

    pub fn dir_crc(&self) -> u16 {
        self.dir_crc
    }

    pub fn find(&self, name: &str) -> Option<&PakEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    pub fn find_case_insensitive(&self, name: &str) -> Option<&PakEntry> {
        self.entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    pub fn read(&self, entry: &PakEntry) -> Result<Vec<u8>, PakError> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(entry.offset as u64))?;

        let mut data = vec![0u8; entry.length as usize];
        file.read_exact(&mut data)?;
        Ok(data)
    }

    pub fn read_by_name(&self, name: &str) -> Result<Vec<u8>, PakError> {
        let entry = self.find(name).ok_or(PakError::EntryNotFound)?;
        self.read(entry)
    }

    pub fn is_stock_pak0(&self) -> bool {
        const PAK0_COUNT: usize = 339;
        const PAK0_CRC: u16 = 52883;
        self.entries.len() == PAK0_COUNT && self.dir_crc == PAK0_CRC
    }
}
