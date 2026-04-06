//! Central command system for tray menu actions
//!
//! Provides communication between tray menu (native macOS) and the Dioxus app.
//! Only used for tray -> app communication; UI actions are handled directly.

use std::sync::Mutex;

/// Commands sent from tray menu to app
#[derive(Debug, Clone)]
pub enum Command {
    /// Set resolution (width, height) - from tray Resolution submenu
    SetResolution(u32, u32),
    /// Set frame rate - from tray Frame Rate submenu
    SetFps(u32),
}

/// Global command queue
static COMMAND_QUEUE: Mutex<Vec<Command>> = Mutex::new(Vec::new());

/// Push a command to the queue (called from tray)
pub fn push_command(cmd: Command) {
    if let Ok(mut queue) = COMMAND_QUEUE.lock() {
        tracing::debug!("[Commands] Pushed: {:?}", cmd);
        queue.push(cmd);
    }
}

/// Take all pending commands from the queue (called from app handler)
pub fn take_commands() -> Vec<Command> {
    if let Ok(mut queue) = COMMAND_QUEUE.lock() {
        std::mem::take(&mut *queue)
    } else {
        Vec::new()
    }
}
