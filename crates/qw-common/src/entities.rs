// Quake entity text parsing.

use crate::com_parse::com_parse;

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
}
