use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::Result;

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub name: String,
}

impl Resolution {
    pub fn new(width: u32, height: u32, name: &str) -> Self {
        Self {
            width,
            height,
            name: name.to_string(),
        }
    }
}

impl Default for Resolution {
    fn default() -> Self {
        Self::new(3440, 1440, "3440x1440 (Ultrawide)")
    }
}

pub const RESOLUTIONS: &[(u32, u32, &str)] = &[
    (1280, 720, "1280x720 (HD)"),
    (1920, 1080, "1920x1080 (FHD)"),
    (2560, 1440, "2560x1440 (QHD)"),
    (3440, 1440, "3440x1440 (Ultrawide)"),
    (3840, 2160, "3840x2160 (4K)"),
    (2560, 1080, "2560x1080 (Ultrawide FHD)"),
    (5120, 1440, "5120x1440 (Super Ultrawide)"),
];

pub const FRAME_RATES: &[u32] = &[30, 60, 120, 144];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResolution {
    pub width: u32,
    pub height: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: u32,
    pub auto_create: bool,
    pub start_at_login: bool,
    pub show_tray_icon: bool,
    pub display_name: String,
    #[serde(default)]
    pub custom_resolutions: Vec<CustomResolution>,
    // Legacy field - kept for backwards compatibility but not used
    #[serde(default)]
    pub hidpi: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 3440,
            height: 1440,
            frame_rate: 60,
            auto_create: true,
            start_at_login: false,
            show_tray_icon: true,
            display_name: "Parsec".to_string(),
            custom_resolutions: Vec::new(),
            hidpi: false,
        }
    }
}

impl Settings {
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vdisplay")
            .join("settings.json")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let settings: Settings = serde_json::from_str(&content)?;
            Ok(settings)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn resolution_display(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}
