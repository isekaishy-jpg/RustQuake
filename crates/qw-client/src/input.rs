use std::collections::HashMap;

use qw_window_glfw::{Action, Key};

#[derive(Debug, Clone)]
pub struct InputBindings {
    bindings: HashMap<Key, Bind>,
}

#[derive(Debug, Clone)]
enum Bind {
    Toggle(String),
    Command(String),
}

impl InputBindings {
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    pub fn bind_toggle(&mut self, key: Key, base: impl Into<String>) {
        self.bindings.insert(key, Bind::Toggle(base.into()));
    }

    pub fn bind_command(&mut self, key: Key, command: impl Into<String>) {
        self.bindings.insert(key, Bind::Command(command.into()));
    }

    pub fn command_for(&self, key: Key, action: Action) -> Option<String> {
        let Some(binding) = self.bindings.get(&key) else {
            return None;
        };

        match (binding, action) {
            (Bind::Toggle(base), Action::Press) => Some(format!("+{base}")),
            (Bind::Toggle(base), Action::Release) => Some(format!("-{base}")),
            (Bind::Command(command), Action::Press) => Some(command.clone()),
            _ => None,
        }
    }
}

impl Default for InputBindings {
    fn default() -> Self {
        let mut bindings = Self::new();
        bindings.bind_toggle(Key::Up, "forward");
        bindings.bind_toggle(Key::Down, "back");
        bindings.bind_toggle(Key::Left, "moveleft");
        bindings.bind_toggle(Key::Right, "moveright");
        bindings.bind_toggle(Key::Space, "jump");
        bindings.bind_command(Key::Enter, "messagemode");
        bindings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_press_to_plus_command() {
        let bindings = InputBindings::default();
        let cmd = bindings.command_for(Key::Up, Action::Press).unwrap();
        assert_eq!(cmd, "+forward");
    }

    #[test]
    fn maps_release_to_minus_command() {
        let bindings = InputBindings::default();
        let cmd = bindings.command_for(Key::Space, Action::Release).unwrap();
        assert_eq!(cmd, "-jump");
    }

    #[test]
    fn ignores_repeat() {
        let bindings = InputBindings::default();
        assert!(bindings.command_for(Key::Up, Action::Repeat).is_none());
    }

    #[test]
    fn command_bind_only_fires_on_press() {
        let mut bindings = InputBindings::new();
        bindings.bind_command(Key::Enter, "impulse 10");
        assert_eq!(
            bindings.command_for(Key::Enter, Action::Press),
            Some("impulse 10".to_string())
        );
        assert!(bindings.command_for(Key::Enter, Action::Release).is_none());
    }
}
