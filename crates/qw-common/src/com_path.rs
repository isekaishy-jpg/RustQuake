// Path helpers mirroring COM_* utilities.

pub fn skip_path(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

pub fn strip_extension(path: &str) -> String {
    let mut out = String::new();
    for ch in path.chars() {
        if ch == '.' {
            break;
        }
        out.push(ch);
    }
    out
}

pub fn file_extension(path: &str) -> String {
    let mut found = false;
    let mut out = String::new();
    for ch in path.chars() {
        if found {
            if out.len() >= 7 {
                break;
            }
            out.push(ch);
        } else if ch == '.' {
            found = true;
        }
    }
    if found {
        out
    } else {
        String::new()
    }
}

pub fn file_base(path: &str) -> String {
    if path.is_empty() {
        return "?model?".to_string();
    }

    let mut end = path.len();
    for (i, ch) in path.char_indices().rev() {
        if ch == '.' {
            end = i;
            break;
        }
        if i == 0 {
            break;
        }
    }

    let mut start = 0;
    for (i, ch) in path[..end].char_indices().rev() {
        if ch == '/' || ch == '\\' {
            start = i + 1;
            break;
        }
        if i == 0 {
            break;
        }
    }

    if end <= start + 1 {
        return "?model?".to_string();
    }

    path[start..end].to_string()
}

pub fn default_extension(path: &str, extension: &str) -> String {
    if has_extension(path) {
        return path.to_string();
    }
    format!("{}{}", path, extension)
}

fn has_extension(path: &str) -> bool {
    let mut chars = path.chars().rev();
    while let Some(ch) = chars.next() {
        if ch == '/' || ch == '\\' {
            return false;
        }
        if ch == '.' {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_path() {
        assert_eq!(skip_path("gfx/char.lmp"), "char.lmp");
        assert_eq!(skip_path("maps\\e1m1.bsp"), "e1m1.bsp");
    }

    #[test]
    fn strips_extension() {
        assert_eq!(strip_extension("maps/e1m1.bsp"), "maps/e1m1");
    }

    #[test]
    fn file_base_extracts() {
        assert_eq!(file_base("models/player.mdl"), "player");
    }

    #[test]
    fn default_extension_adds() {
        assert_eq!(default_extension("config", ".cfg"), "config.cfg");
        assert_eq!(default_extension("config.cfg", ".cfg"), "config.cfg");
    }
}
