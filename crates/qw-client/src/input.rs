use std::collections::HashMap;

use qw_common::UserCmd;
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
        let binding = self.bindings.get(&key)?;

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
        bindings.bind_toggle(Key::Shift, "speed");
        bindings.bind_toggle(Key::Mouse1, "attack");
        bindings.bind_command(Key::Enter, "messagemode");
        bindings
    }
}

const FORWARD_SPEED: i16 = 200;
const BACK_SPEED: i16 = 200;
const SIDE_SPEED: i16 = 350;
const UP_SPEED: i16 = 200;
const SPEED_MULT: i32 = 2;
const BUTTON_ATTACK: u8 = 1;
const BUTTON_JUMP: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandTarget {
    Local,
    Server,
}

#[derive(Debug, Default, Clone)]
pub struct InputState {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    jump: bool,
    attack: bool,
    speed: bool,
    impulse: Option<u8>,
}

impl InputState {
    pub fn apply_command(&mut self, command: &str) -> CommandTarget {
        let trimmed = command.trim();
        if trimmed.is_empty() {
            return CommandTarget::Local;
        }

        if let Some(rest) = trimmed.strip_prefix('+') {
            return self.handle_toggle(rest, true);
        }
        if let Some(rest) = trimmed.strip_prefix('-') {
            return self.handle_toggle(rest, false);
        }

        if let Some(rest) = trimmed.strip_prefix("impulse") {
            let value = rest.trim();
            if let Ok(parsed) = value.parse::<u8>() {
                self.impulse = Some(parsed);
            }
            return CommandTarget::Local;
        }

        match trimmed {
            "messagemode" => CommandTarget::Local,
            _ => CommandTarget::Server,
        }
    }

    pub fn build_usercmd(&mut self) -> UserCmd {
        let mut cmd = UserCmd::default();
        if self.forward {
            cmd.forwardmove = cmd.forwardmove.saturating_add(FORWARD_SPEED);
        }
        if self.back {
            cmd.forwardmove = cmd.forwardmove.saturating_sub(BACK_SPEED);
        }
        if self.right {
            cmd.sidemove = cmd.sidemove.saturating_add(SIDE_SPEED);
        }
        if self.left {
            cmd.sidemove = cmd.sidemove.saturating_sub(SIDE_SPEED);
        }
        if self.up {
            cmd.upmove = cmd.upmove.saturating_add(UP_SPEED);
        }
        if self.down {
            cmd.upmove = cmd.upmove.saturating_sub(UP_SPEED);
        }
        if self.jump {
            cmd.buttons |= BUTTON_JUMP;
        }
        if self.attack {
            cmd.buttons |= BUTTON_ATTACK;
        }
        if self.speed {
            scale_move(&mut cmd, SPEED_MULT);
        }
        if let Some(impulse) = self.impulse.take() {
            cmd.impulse = impulse;
        }
        cmd
    }

    fn handle_toggle(&mut self, base: &str, pressed: bool) -> CommandTarget {
        match base {
            "forward" => self.forward = pressed,
            "back" => self.back = pressed,
            "moveleft" => self.left = pressed,
            "moveright" => self.right = pressed,
            "moveup" => self.up = pressed,
            "movedown" => self.down = pressed,
            "jump" => self.jump = pressed,
            "attack" => self.attack = pressed,
            "speed" => self.speed = pressed,
            _ => return CommandTarget::Server,
        }
        CommandTarget::Local
    }
}

fn scale_move(cmd: &mut UserCmd, scale: i32) {
    cmd.forwardmove =
        ((cmd.forwardmove as i32) * scale).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    cmd.sidemove = ((cmd.sidemove as i32) * scale).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    cmd.upmove = ((cmd.upmove as i32) * scale).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
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

    #[test]
    fn applies_toggle_commands_to_state() {
        let mut state = InputState::default();
        assert_eq!(state.apply_command("+forward"), CommandTarget::Local);
        assert_eq!(state.build_usercmd().forwardmove, FORWARD_SPEED);
        assert_eq!(state.apply_command("-forward"), CommandTarget::Local);
        assert_eq!(state.build_usercmd().forwardmove, 0);
    }

    #[test]
    fn jump_sets_button_bit() {
        let mut state = InputState::default();
        state.apply_command("+jump");
        let cmd = state.build_usercmd();
        assert_eq!(cmd.buttons & BUTTON_JUMP, BUTTON_JUMP);
    }

    #[test]
    fn attack_sets_button_bit() {
        let mut state = InputState::default();
        state.apply_command("+attack");
        let cmd = state.build_usercmd();
        assert_eq!(cmd.buttons & BUTTON_ATTACK, BUTTON_ATTACK);
    }

    #[test]
    fn impulse_sets_cmd_value() {
        let mut state = InputState::default();
        assert_eq!(state.apply_command("impulse 5"), CommandTarget::Local);
        let cmd = state.build_usercmd();
        assert_eq!(cmd.impulse, 5);
    }

    #[test]
    fn forwards_unknown_commands() {
        let mut state = InputState::default();
        assert_eq!(state.apply_command("say hello"), CommandTarget::Server);
    }
}
