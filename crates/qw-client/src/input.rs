use qw_window_glfw::{Action, Key};

pub fn map_key_action(key: Key, action: Action) -> Option<String> {
    let base = match key {
        Key::Up => "forward",
        Key::Down => "back",
        Key::Left => "moveleft",
        Key::Right => "moveright",
        Key::Space => "jump",
        _ => return None,
    };

    match action {
        Action::Press => Some(format!("+{base}")),
        Action::Release => Some(format!("-{base}")),
        Action::Repeat => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_press_to_plus_command() {
        let cmd = map_key_action(Key::Up, Action::Press).unwrap();
        assert_eq!(cmd, "+forward");
    }

    #[test]
    fn maps_release_to_minus_command() {
        let cmd = map_key_action(Key::Space, Action::Release).unwrap();
        assert_eq!(cmd, "-jump");
    }

    #[test]
    fn ignores_repeat() {
        assert!(map_key_action(Key::Up, Action::Repeat).is_none());
    }
}
