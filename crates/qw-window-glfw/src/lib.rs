#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub title: String,
    pub resizable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            title: "RustQuake".to_string(),
            resizable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,
    Shift,
    Ctrl,
    Alt,
    Up,
    Down,
    Left,
    Right,
    Mouse1,
    Mouse2,
    Mouse3,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Press,
    Release,
    Repeat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowEvent {
    CloseRequested,
    Resized(u32, u32),
    Key { key: Key, action: Action },
}

#[cfg(feature = "glfw")]
mod glfw_backend;
#[cfg(feature = "glfw")]
pub use glfw_backend::GlfwWindow;

#[cfg(not(feature = "glfw"))]
mod stub;
#[cfg(not(feature = "glfw"))]
pub use stub::GlfwWindow;

#[cfg(all(test, not(feature = "glfw")))]
mod tests {
    use super::*;

    #[test]
    fn closes_window() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        assert!(!window.should_close());
        window.close();
        assert!(window.should_close());
    }

    #[test]
    fn updates_title() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        window.set_title("Unit");
        assert_eq!(window.config().title, "Unit");
    }

    #[test]
    fn collects_pending_events() {
        let mut window = GlfwWindow::new(WindowConfig::default());
        window.push_event(WindowEvent::CloseRequested);
        let events = window.poll_events();
        assert_eq!(events, vec![WindowEvent::CloseRequested]);
        assert!(window.poll_events().is_empty());
    }
}
