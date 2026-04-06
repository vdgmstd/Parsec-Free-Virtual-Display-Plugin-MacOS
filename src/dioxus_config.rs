//! PVDisplayPlugin.toml configuration reader
//!
//! Embeds configuration from PVDisplayPlugin.toml at compile time.
//! Only parses fields needed for runtime; build-time fields (bundle, out_dir, etc.)
//! are handled by build.sh directly.

use anyhow::Result;
use serde::Deserialize;

/// Embedded config from PVDisplayPlugin.toml (compile-time)
const CONFIG_TOML: &str = include_str!("../PVDisplayPlugin.toml");

/// Root configuration - only runtime-relevant sections
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DioxusConfig {
    pub application: ApplicationConfig,
    pub desktop: DesktopConfig,
    pub bundle: BundleConfig,
}

/// Bundle configuration (only identifier needed at runtime for LaunchAgent)
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct BundleConfig {
    pub identifier: Option<String>,
}

/// Application metadata used at runtime
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct ApplicationConfig {
    /// App display name (shown in window title)
    pub name: Option<String>,
    /// App version (shown in titlebar)
    pub version: Option<String>,
}

/// Desktop window configuration
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct DesktopConfig {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub resizable: Option<bool>,
    pub visible: Option<bool>,
}

impl DioxusConfig {
    /// Load configuration from embedded PVDisplayPlugin.toml
    pub fn load() -> Result<Self> {
        let config: DioxusConfig = toml::from_str(CONFIG_TOML)?;
        Ok(config)
    }

    /// Get application name for window title
    pub fn app_name(&self) -> String {
        self.application
            .name
            .clone()
            .unwrap_or_else(|| "PVDisplayPlugin".to_string())
    }

    /// Get desktop config
    pub fn desktop_config(&self) -> &DesktopConfig {
        &self.desktop
    }

    /// Get bundle identifier (for LaunchAgent label)
    pub fn identifier(&self) -> String {
        self.bundle
            .identifier
            .clone()
            .unwrap_or_else(|| "com.vdgmstd.parsec-vdisplay".to_string())
    }
}

impl DesktopConfig {
    pub fn width(&self) -> f64 {
        self.width.unwrap_or(450) as f64
    }

    pub fn height(&self) -> f64 {
        self.height.unwrap_or(570) as f64
    }

    pub fn min_width(&self) -> Option<f64> {
        self.min_width.map(|w| w as f64)
    }

    pub fn min_height(&self) -> Option<f64> {
        self.min_height.map(|h| h as f64)
    }

    pub fn max_width(&self) -> Option<f64> {
        self.max_width.map(|w| w as f64)
    }

    pub fn max_height(&self) -> Option<f64> {
        self.max_height.map(|h| h as f64)
    }

    pub fn resizable(&self) -> bool {
        self.resizable.unwrap_or(false)
    }

    pub fn visible(&self) -> bool {
        self.visible.unwrap_or(true)
    }
}
