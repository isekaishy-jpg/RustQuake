use std::sync::mpsc::Receiver;

use crate::{Action, Key, WindowConfig, WindowEvent};

pub struct GlfwWindow {
    config: WindowConfig,
    glfw: glfw::Glfw,
    window: glfw::Window,
    events: Receiver<(f64, glfw::WindowEvent)>,
    pending_events: Vec<WindowEvent>,
}

impl GlfwWindow {
    pub fn new(config: WindowConfig) -> Self {
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).expect("failed to init GLFW");
        glfw.window_hint(glfw::WindowHint::Resizable(config.resizable));
        glfw.window_hint(glfw::WindowHint::ContextVersion(3, 3));
        glfw.window_hint(glfw::WindowHint::OpenGlProfile(
            glfw::OpenGlProfileHint::Core,
        ));
        glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));

        let (mut window, events) = glfw
            .create_window(
                config.width,
                config.height,
                &config.title,
                glfw::WindowMode::Windowed,
            )
            .expect("failed to create GLFW window");
        window.set_key_polling(true);
        window.set_mouse_button_polling(true);
        window.set_framebuffer_size_polling(true);
        window.set_close_polling(true);
        window.make_current();

        Self {
            config,
            glfw,
            window,
            events,
            pending_events: Vec::new(),
        }
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        self.glfw.poll_events();
        for (_, event) in glfw::flush_messages(&self.events) {
            if let Some(mapped) = map_event(event) {
                if let WindowEvent::Resized(width, height) = mapped {
                    self.config.width = width;
                    self.config.height = height;
                }
                self.pending_events.push(mapped);
            }
        }
        std::mem::take(&mut self.pending_events)
    }

    pub fn push_event(&mut self, event: WindowEvent) {
        if let WindowEvent::Resized(width, height) = event {
            self.config.width = width;
            self.config.height = height;
        }
        self.pending_events.push(event);
    }

    pub fn should_close(&self) -> bool {
        self.window.should_close()
    }

    pub fn close(&mut self) {
        self.window.set_should_close(true);
    }

    pub fn size(&self) -> (u32, u32) {
        let (width, height) = self.window.get_framebuffer_size();
        (width.max(1) as u32, height.max(1) as u32)
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        let title = title.into();
        self.window.set_title(&title);
        self.config.title = title;
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }
}

fn map_event(event: glfw::WindowEvent) -> Option<WindowEvent> {
    match event {
        glfw::WindowEvent::Close => Some(WindowEvent::CloseRequested),
        glfw::WindowEvent::FramebufferSize(width, height)
        | glfw::WindowEvent::Size(width, height) => Some(WindowEvent::Resized(
            width.max(1) as u32,
            height.max(1) as u32,
        )),
        glfw::WindowEvent::Key(key, scancode, action, _) => Some(WindowEvent::Key {
            key: map_key(key, scancode),
            action: map_action(action),
        }),
        glfw::WindowEvent::MouseButton(button, action, _) => {
            map_mouse_button(button).map(|key| WindowEvent::Key {
                key,
                action: map_action(action),
            })
        }
        _ => None,
    }
}

fn map_action(action: glfw::Action) -> Action {
    match action {
        glfw::Action::Press => Action::Press,
        glfw::Action::Release => Action::Release,
        glfw::Action::Repeat => Action::Repeat,
    }
}

fn map_key(key: glfw::Key, scancode: i32) -> Key {
    match key {
        glfw::Key::Escape => Key::Escape,
        glfw::Key::Enter | glfw::Key::KpEnter => Key::Enter,
        glfw::Key::Space => Key::Space,
        glfw::Key::Tab => Key::Tab,
        glfw::Key::Backspace => Key::Backspace,
        glfw::Key::LeftShift | glfw::Key::RightShift => Key::Shift,
        glfw::Key::LeftControl | glfw::Key::RightControl => Key::Ctrl,
        glfw::Key::LeftAlt | glfw::Key::RightAlt => Key::Alt,
        glfw::Key::Up => Key::Up,
        glfw::Key::Down => Key::Down,
        glfw::Key::Left => Key::Left,
        glfw::Key::Right => Key::Right,
        glfw::Key::Unknown => Key::Other(scancode.max(0) as u16),
        other => Key::Other(other as u16),
    }
}

fn map_mouse_button(button: glfw::MouseButton) -> Option<Key> {
    match button {
        glfw::MouseButton::Button1 => Some(Key::Mouse1),
        glfw::MouseButton::Button2 => Some(Key::Mouse2),
        glfw::MouseButton::Button3 => Some(Key::Mouse3),
        _ => None,
    }
}

#[cfg(all(test, feature = "glfw"))]
mod tests {
    use super::*;

    #[test]
    fn maps_known_keys() {
        assert_eq!(map_key(glfw::Key::Escape, 0), Key::Escape);
        assert_eq!(map_key(glfw::Key::LeftShift, 0), Key::Shift);
        assert_eq!(map_key(glfw::Key::RightAlt, 0), Key::Alt);
    }

    #[test]
    fn maps_mouse_buttons() {
        assert_eq!(
            map_mouse_button(glfw::MouseButton::Button1),
            Some(Key::Mouse1)
        );
        assert_eq!(
            map_mouse_button(glfw::MouseButton::Button2),
            Some(Key::Mouse2)
        );
    }
}
