use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub threads_per_download: u8,
    pub max_concurrent_downloads: u8,
    pub download_dir: String,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let download_dir = dirs::download_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .to_string_lossy()
            .to_string();
        Self {
            threads_per_download: 4,
            max_concurrent_downloads: 3,
            download_dir,
            quiet_hours_enabled: false,
            quiet_hours_start: None,
            quiet_hours_end: None,
        }
    }
}

fn settings_path() -> Result<PathBuf> {
    let base = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
    let dir = base.join("rdtool");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("settings.json"))
}

pub fn load_settings() -> AppSettings {
    settings_path()
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let json = serde_json::to_string_pretty(settings)?;
    std::fs::write(settings_path()?, json)?;
    Ok(())
}
