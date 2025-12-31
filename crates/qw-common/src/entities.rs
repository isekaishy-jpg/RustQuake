// Quake entity text parsing.

use crate::com_parse::com_parse;
use std::path::Path;

#[derive(Debug)]
pub enum EntityError {
    UnexpectedToken(String),
    UnexpectedEof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pairs: Vec<(String, String)>,
}

impl Entity {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }
}

pub fn parse_entities(text: &str) -> Result<Vec<Entity>, EntityError> {
    let mut entities = Vec::new();
    let mut input = text;
    while let Some((token, rest)) = com_parse(input) {
        input = rest;
        if token != "{" {
            return Err(EntityError::UnexpectedToken(token));
        }

        let mut pairs = Vec::new();
        loop {
            let (key, rest) = com_parse(input).ok_or(EntityError::UnexpectedEof)?;
            input = rest;
            if key == "}" {
                break;
            }
            let (value, rest) = com_parse(input).ok_or(EntityError::UnexpectedEof)?;
            input = rest;
            if value == "}" {
                return Err(EntityError::UnexpectedToken(value));
            }
            pairs.push((key, value));
        }

        entities.push(Entity { pairs });
    }

    Ok(entities)
}

pub fn worldspawn_wad_list(entities: &[Entity]) -> Vec<String> {
    let Some(worldspawn) = entities
        .iter()
        .find(|entity| entity.get("classname") == Some("worldspawn"))
    else {
        return Vec::new();
    };
    let Some(wads) = worldspawn.get("wad") else {
        return Vec::new();
    };

    wads.split(';')
        .filter_map(|entry| {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return None;
            }
            let name = Path::new(trimmed)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or(trimmed);
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entities_and_values() {
        let text = "{\n\"classname\" \"worldspawn\"\n\"wad\" \"foo.wad\"\n}\n{\n\"classname\" \"info\"\n\"angle\" \"90\"\n}\n";
        let entities = parse_entities(text).unwrap();
        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].get("classname"), Some("worldspawn"));
        assert_eq!(entities[0].get("wad"), Some("foo.wad"));
        assert_eq!(entities[1].get("angle"), Some("90"));
    }

    #[test]
    fn errors_on_missing_closing_brace() {
        let text = "{\n\"classname\" \"worldspawn\"\n";
        let err = parse_entities(text).unwrap_err();
        matches!(err, EntityError::UnexpectedEof);
    }

    #[test]
    fn parses_wad_list_from_worldspawn() {
        let text = "{\n\"classname\" \"worldspawn\"\n\"wad\" \"C:\\\\quake\\\\id1\\\\gfx.wad;foo.wad;\"\n}\n";
        let entities = parse_entities(text).unwrap();
        let wads = worldspawn_wad_list(&entities);
        assert_eq!(wads, vec!["gfx.wad", "foo.wad"]);
    }
}
