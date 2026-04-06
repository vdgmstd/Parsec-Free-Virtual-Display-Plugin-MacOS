//! Parsec Free VDisplay Plugin
//!
//! Automatically creates and destroys virtual displays when Parsec clients
//! connect and disconnect.

mod app;
mod autostart;
mod commands;
mod dioxus_config;
mod display;
mod parsec;
mod settings;
mod tray;

use anyhow::Result;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder, WindowCloseBehaviour};
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

use crate::app::App;
use crate::dioxus_config::DioxusConfig;
use crate::settings::Settings;

fn main() -> Result<()> {
    // Initialize logging
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .compact()
        .init();

    info!("Parsec Free VDisplay Plugin starting...");

    // Load PVDisplayPlugin.toml configuration (embedded at compile time)
    let dioxus_cfg = DioxusConfig::load().unwrap_or_else(|e| {
        warn!("Failed to parse config: {}, using defaults", e);
        DioxusConfig::default()
    });
    let desktop_cfg = dioxus_cfg.desktop_config();
    let app_name = dioxus_cfg.app_name();

    info!(
        "Window config: {}x{}, resizable={}, name={}",
        desktop_cfg.width(),
        desktop_cfg.height(),
        desktop_cfg.resizable(),
        app_name
    );

    // Load settings
    let settings = Settings::load().unwrap_or_default();
    info!(
        "Settings loaded: {}x{} @ {}Hz",
        settings.width, settings.height, settings.frame_rate
    );

    // Build window with config from Dioxus.toml
    let mut window_builder = WindowBuilder::new()
        .with_title(&app_name)
        .with_inner_size(LogicalSize::new(desktop_cfg.width(), desktop_cfg.height()))
        .with_resizable(desktop_cfg.resizable())
        .with_visible(desktop_cfg.visible())
        // Custom titlebar requires these
        .with_decorations(false)
        .with_transparent(true);

    // Apply min size if specified
    if let (Some(min_w), Some(min_h)) = (desktop_cfg.min_width(), desktop_cfg.min_height()) {
        window_builder = window_builder.with_min_inner_size(LogicalSize::new(min_w, min_h));
    }

    // Apply max size if specified
    if let (Some(max_w), Some(max_h)) = (desktop_cfg.max_width(), desktop_cfg.max_height()) {
        window_builder = window_builder.with_max_inner_size(LogicalSize::new(max_w, max_h));
    }

    // Configure Dioxus desktop
    let config = Config::new()
        .with_window(window_builder)
        // Hide window on close (not destroy) so it can be reopened via tray
        .with_close_behaviour(WindowCloseBehaviour::WindowHides);

    // Launch Dioxus app with config
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(App);

    info!("Parsec Free VDisplay Plugin shutting down...");

    Ok(())
}
