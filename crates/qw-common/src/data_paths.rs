// Data path discovery for local Quake installs.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum DataPathError {
    NotFound,
    Io(std::io::Error),
}

impl From<std::io::Error> for DataPathError {
    fn from(err: std::io::Error) -> Self {
        DataPathError::Io(err)
    }
}

pub fn locate_data_dir() -> Result<PathBuf, DataPathError> {
    if let Ok(value) = env::var("RUSTQUAKE_DATA_DIR") {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    let cwd = env::current_dir()?;
    let config_path = cwd.join("config").join("data_paths.toml");
    if config_path.exists() {
        let contents = fs::read_to_string(&config_path)?;
        if let Some(dir) = parse_quake_dir(&contents) {
            return Ok(PathBuf::from(dir));
        }
    }

    if let Some(dir) = find_default_quake_dir() {
        return Ok(dir);
    }

    Err(DataPathError::NotFound)
}

pub fn find_id1_dir(data_dir: &Path) -> Option<PathBuf> {
    find_game_dir(data_dir, "id1")
}

pub fn find_game_dir(data_dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = data_dir.join(name);
    if direct.is_dir() {
        return Some(direct);
    }

    let rerelease = data_dir.join("rerelease").join(name);
    if rerelease.is_dir() {
        return Some(rerelease);
    }

    None
}

fn parse_quake_dir(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if !line.starts_with("quake_dir") {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let _key = parts.next()?;
        let raw_value = parts.next()?.trim();
        let trimmed = raw_value.trim_matches('"').trim_matches('\'').trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    None
}

fn find_default_quake_dir() -> Option<PathBuf> {
    for path in default_quake_paths() {
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

fn default_quake_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if cfg!(windows) {
        if let Ok(pf86) = env::var("ProgramFiles(x86)") {
            paths.push(PathBuf::from(pf86).join("Steam\\steamapps\\common\\Quake"));
        }
        if let Ok(pf) = env::var("ProgramFiles") {
            paths.push(PathBuf::from(pf).join("Steam\\steamapps\\common\\Quake"));
        }
    } else if cfg!(unix)
        && let Ok(home) = env::var("HOME")
    {
        let home = PathBuf::from(home);
        paths.push(home.join(".steam/steam/steamapps/common/Quake"));
        paths.push(home.join(".steam/root/steamapps/common/Quake"));
        paths.push(home.join(".local/share/Steam/steamapps/common/Quake"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::{locate_data_dir, parse_quake_dir};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn parses_quake_dir_value() {
        let input = r#"
            # comment
            quake_dir = "C:/Games/Quake"
        "#;
        assert_eq!(parse_quake_dir(input).as_deref(), Some("C:/Games/Quake"));
    }

    #[test]
    fn ignores_missing_quake_dir() {
        let input = "other = \"value\"";
        assert!(parse_quake_dir(input).is_none());
    }

    #[test]
    fn locate_data_dir_uses_config() {
        let _guard = ENV_LOCK.lock().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        let base = std::env::temp_dir().join("rustquake-data-path-test");
        let config_dir = base.join("config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let data_dir = base.join("quake");
        std::fs::create_dir_all(&data_dir).unwrap();
        let config_path = config_dir.join("data_paths.toml");
        let contents = format!(
            "quake_dir = \"{}\"",
            data_dir.to_string_lossy().replace('\\', "/")
        );
        std::fs::write(&config_path, contents).unwrap();

        std::env::set_current_dir(&base).unwrap();
        let found = locate_data_dir().unwrap();
        assert_eq!(found, data_dir);

        std::env::set_current_dir(&original_dir).unwrap();
        std::fs::remove_dir_all(base).ok();
    }
}
