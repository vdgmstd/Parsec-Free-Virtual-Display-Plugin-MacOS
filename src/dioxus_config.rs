//! Cargo.toml configuration reader
//!
//! Reads app configuration from Cargo.toml [package.metadata.*] sections at compile time.

use anyhow::Result;
use serde::Deserialize;

/// Embedded config from Cargo.toml (compile-time)
const CONFIG_TOML: &str = include_str!("../Cargo.toml");

/// Root Cargo.toml structure
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct CargoConfig {
    pub package: PackageConfig,
}

/// [package] section
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PackageConfig {
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub metadata: MetadataConfig,
}

/// [package.metadata] section
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct MetadataConfig {
    pub app: AppConfig,
    pub desktop: DesktopConfig,
    pub bundle: BundleConfig,
}

/// [package.metadata.app] section
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct AppConfig {
    pub display_name: Option<String>,
    pub out_dir: Option<String>,
    pub asset_dir: Option<String>,
}

/// [package.metadata.desktop] section
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
}

/// [package.metadata.bundle] section
#[derive(Debug, Deserialize, Default, Clone)]
#[serde(default)]
pub struct BundleConfig {
    pub identifier: Option<String>,
    pub publisher: Option<String>,
    pub icon: Option<String>,
    pub category: Option<String>,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub minimum_system_version: Option<String>,
}

/// Main config interface (compatibility wrapper)
#[derive(Debug, Clone)]
pub struct DioxusConfig {
    cargo: CargoConfig,
}

impl Default for DioxusConfig {
    fn default() -> Self {
        Self {
            cargo: CargoConfig::default(),
        }
    }
}

impl DioxusConfig {
    /// Load configuration from embedded Cargo.toml
    pub fn load() -> Result<Self> {
        let cargo: CargoConfig = toml::from_str(CONFIG_TOML)?;
        Ok(Self { cargo })
    }

    /// Get app version from [package] section
    pub fn version(&self) -> Option<String> {
        self.cargo.package.version.clone()
    }

    /// Get application display name
    pub fn app_name(&self) -> String {
        self.cargo
            .package
            .metadata
            .app
            .display_name
            .clone()
            .unwrap_or_else(|| "PVDisplayPlugin".to_string())
    }

    /// Get desktop config
    pub fn desktop_config(&self) -> &DesktopConfig {
        &self.cargo.package.metadata.desktop
    }

    /// Get bundle identifier (for LaunchAgent label)
    pub fn identifier(&self) -> String {
        self.cargo
            .package
            .metadata
            .bundle
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
        true
    }
}
