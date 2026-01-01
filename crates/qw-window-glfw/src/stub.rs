use crate::{WindowConfig, WindowEvent};
use std::ffi::c_void;

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

    pub fn push_event(&mut self, event: WindowEvent) {
        self.pending_events.push(event);
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

    pub fn swap_buffers(&mut self) {}

    pub fn get_proc_address(&self, _symbol: &str) -> *const c_void {
        std::ptr::null()
    }
}
