//! Auto-start management via macOS LaunchAgents

#![allow(dead_code)]

use anyhow::{Result, anyhow};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::{info, error};

use crate::dioxus_config::DioxusConfig;

pub struct AutoStart {
    label: String,
    plist_path: PathBuf,
    app_path: PathBuf,
}

impl AutoStart {
    pub fn new() -> Result<Self> {
        // Load identifier from Dioxus.toml
        let config = DioxusConfig::load().unwrap_or_default();
        let label = config.identifier();

        Self::with_label(&label)
    }

    pub fn with_label(label: &str) -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("Cannot find home directory"))?;
        let plist_path = home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", label));

        // Get current executable path
        let app_path = std::env::current_exe()?;

        Ok(Self {
            label: label.to_string(),
            plist_path,
            app_path,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.plist_path.exists()
    }

    pub fn enable(&self) -> Result<()> {
        info!("Enabling auto-start with label: {}", self.label);

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <false/>
    <key>StandardOutPath</key>
    <string>/tmp/vdisplay.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/vdisplay.error.log</string>
</dict>
</plist>
"#,
            self.label,
            self.app_path.display()
        );

        // Ensure LaunchAgents directory exists
        if let Some(parent) = self.plist_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write plist file
        fs::write(&self.plist_path, plist_content)?;

        // Load the agent
        self.load_agent()?;

        info!("Auto-start enabled: {:?}", self.plist_path);
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        info!("Disabling auto-start");

        // Unload the agent first
        let _ = self.unload_agent();

        // Remove plist file
        if self.plist_path.exists() {
            fs::remove_file(&self.plist_path)?;
        }

        info!("Auto-start disabled");
        Ok(())
    }

    pub fn toggle(&self) -> Result<bool> {
        if self.is_enabled() {
            self.disable()?;
            Ok(false)
        } else {
            self.enable()?;
            Ok(true)
        }
    }

    fn load_agent(&self) -> Result<()> {
        let output = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&self.plist_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to load launch agent: {}", stderr);
            // Don't return error as it might already be loaded
        }

        Ok(())
    }

    fn unload_agent(&self) -> Result<()> {
        let output = Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(&self.plist_path)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Failed to unload launch agent: {}", stderr);
            // Don't return error as it might not be loaded
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_path() {
        let autostart = AutoStart::with_label("com.test.app").unwrap();
        assert!(autostart.plist_path.to_string_lossy().contains("LaunchAgents"));
        assert!(autostart.plist_path.to_string_lossy().contains("com.test.app"));
    }
}
