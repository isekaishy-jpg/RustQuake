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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,
    Up,
    Down,
    Left,
    Right,
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

#[derive(Debug)]
pub struct GlfwWindow {
    config: WindowConfig,
    open: bool,
    pending_events: Vec<WindowEvent>,
}

impl GlfwWindow {
    pub fn new(config: WindowConfig) -> Self {
        Self {
            config,
            open: true,
            pending_events: Vec::new(),
        }
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn should_close(&self) -> bool {
        !self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn set_title(&mut self, title: impl Into<String>) {
        self.config.title = title.into();
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }
}

#[cfg(test)]
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
}
