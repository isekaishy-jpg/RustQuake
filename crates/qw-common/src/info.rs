// Quake info string helpers (\key\value pairs).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InfoError {
    DisallowedStarKey,
    InvalidKey,
    InvalidValue,
    LengthExceeded,
}

impl fmt::Display for InfoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InfoError::DisallowedStarKey => write!(f, "star keys are not allowed"),
            InfoError::InvalidKey => write!(f, "invalid key"),
            InfoError::InvalidValue => write!(f, "invalid value"),
            InfoError::LengthExceeded => write!(f, "info string length exceeded"),
        }
    }
}

pub fn value_for_key(info: &str, key: &str) -> Option<String> {
    if key.is_empty() {
        return None;
    }
    let mut iter = info.trim_start_matches('\\').split('\\');
    while let Some(k) = iter.next() {
        if let Some(v) = iter.next() {
            if k == key {
                return Some(v.to_string());
            }
        } else {
            break;
        }
    }
    None
}

pub fn remove_key(info: &mut String, key: &str) {
    let entries = parse_entries(info);
    info.clear();
    for (k, v) in entries {
        if k == key {
            continue;
        }
        info.push('\\');
        info.push_str(&k);
        info.push('\\');
        info.push_str(&v);
    }
}

pub fn remove_prefixed_keys(info: &mut String, prefix: char) {
    let entries = parse_entries(info);
    info.clear();
    for (k, v) in entries {
        if k.starts_with(prefix) {
            continue;
        }
        info.push('\\');
        info.push_str(&k);
        info.push('\\');
        info.push_str(&v);
    }
}

pub fn set_value_for_key(
    info: &mut String,
    key: &str,
    value: &str,
    maxsize: usize,
) -> Result<(), InfoError> {
    if key.starts_with('*') {
        return Err(InfoError::DisallowedStarKey);
    }

    set_value_for_star_key(info, key, value, maxsize)
}

pub fn set_value_for_star_key(
    info: &mut String,
    key: &str,
    value: &str,
    maxsize: usize,
) -> Result<(), InfoError> {
    validate_key_value(key, value)?;

    let cleaned = sanitize_value(key, value);
    if cleaned.is_empty() {
        remove_key(info, key);
        return Ok(());
    }

    let current_value = value_for_key(info, key);
    if let Some(existing) = current_value {
        let new_len = info.len() - existing.len() + cleaned.len();
        if new_len > maxsize {
            return Err(InfoError::LengthExceeded);
        }
    }

    remove_key(info, key);

    let appended_len = info.len() + key.len() + cleaned.len() + 2;
    if appended_len > maxsize {
        return Err(InfoError::LengthExceeded);
    }

    info.push('\\');
    info.push_str(key);
    info.push('\\');
    info.push_str(&cleaned);
    Ok(())
}

#[derive(Debug, Clone)]
pub struct InfoString {
    value: String,
    maxsize: usize,
}

impl InfoString {
    pub fn new(maxsize: usize) -> Self {
        Self {
            value: String::new(),
            maxsize,
        }
    }

    pub fn from_raw(raw: &str, maxsize: usize) -> Self {
        let mut value = raw.to_string();
        if value.len() > maxsize {
            value.truncate(maxsize);
        }
        Self { value, maxsize }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), InfoError> {
        set_value_for_key(&mut self.value, key, value, self.maxsize)
    }

    pub fn set_star(&mut self, key: &str, value: &str) -> Result<(), InfoError> {
        set_value_for_star_key(&mut self.value, key, value, self.maxsize)
    }

    pub fn remove(&mut self, key: &str) {
        remove_key(&mut self.value, key);
    }

    pub fn set_raw(&mut self, raw: &str) {
        self.value.clear();
        self.value.push_str(raw);
        if self.value.len() > self.maxsize {
            self.value.truncate(self.maxsize);
        }
    }
}

fn validate_key_value(key: &str, value: &str) -> Result<(), InfoError> {
    if key.is_empty() || key.len() > 63 {
        return Err(InfoError::InvalidKey);
    }
    if value.len() > 63 {
        return Err(InfoError::InvalidValue);
    }
    if key.contains('\\') || key.contains('"') {
        return Err(InfoError::InvalidKey);
    }
    if value.contains('\\') || value.contains('"') {
        return Err(InfoError::InvalidValue);
    }
    Ok(())
}

fn sanitize_value(key: &str, value: &str) -> String {
    let lower_team = key.eq_ignore_ascii_case("team");
    let allow_high = key.eq_ignore_ascii_case("name");
    let mut out = String::new();
    for ch in value.chars() {
        let mut c = ch as u32;
        if !allow_high {
            c &= 0x7F;
        }
        if c < 32 || c > 127 {
            continue;
        }
        let mut ch = char::from_u32(c).unwrap_or('_');
        if lower_team {
            ch = ch.to_ascii_lowercase();
        }
        if c > 13 {
            out.push(ch);
        }
    }
    out
}

fn parse_entries(info: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut iter = info.trim_start_matches('\\').split('\\');
    while let Some(k) = iter.next() {
        if let Some(v) = iter.next() {
            entries.push((k.to_string(), v.to_string()));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_value() {
        let mut info = String::new();
        set_value_for_key(&mut info, "name", "Player", 128).unwrap();
        assert_eq!(value_for_key(&info, "name"), Some("Player".to_string()));
    }

    #[test]
    fn removes_key() {
        let mut info = String::from("\\name\\Player\\team\\Red");
        remove_key(&mut info, "name");
        assert_eq!(value_for_key(&info, "name"), None);
        assert_eq!(value_for_key(&info, "team"), Some("Red".to_string()));
    }

    #[test]
    fn disallows_star_key() {
        let mut info = String::new();
        let err = set_value_for_key(&mut info, "*ver", "1", 128).unwrap_err();
        assert_eq!(err, InfoError::DisallowedStarKey);
    }

    #[test]
    fn lowercases_team_value() {
        let mut info = String::new();
        set_value_for_key(&mut info, "team", "RED", 128).unwrap();
        assert_eq!(value_for_key(&info, "team"), Some("red".to_string()));
    }

    #[test]
    fn info_string_set_and_remove() {
        let mut info = InfoString::new(128);
        info.set("name", "Player").unwrap();
        info.set_star("*ver", "1").unwrap();
        assert!(info.as_str().contains("\\name\\Player"));
        assert!(info.as_str().contains("\\*ver\\1"));

        info.remove("name");
        assert!(!info.as_str().contains("\\name\\Player"));
    }

    #[test]
    fn info_string_raw_truncates() {
        let mut info = InfoString::new(4);
        info.set_raw("12345");
        assert_eq!(info.as_str(), "1234");
    }
}
