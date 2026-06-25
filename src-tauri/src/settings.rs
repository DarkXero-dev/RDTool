use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FolderRules {
    #[serde(default)]
    pub video: Option<String>,
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub archive: Option<String>,
    #[serde(default)]
    pub programs: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub threads_per_download: u8,
    pub max_concurrent_downloads: u8,
    pub download_dir: String,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_start: Option<String>,
    pub quiet_hours_end: Option<String>,
    #[serde(default)]
    pub tray_enabled: bool,
    #[serde(default)]
    pub folder_rules: FolderRules,
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
            tray_enabled: false,
            folder_rules: FolderRules::default(),
        }
    }
}

pub enum FileCategory {
    Video,
    Audio,
    Archive,
    Programs,
    Other,
}

pub fn detect_file_category(filename: &str) -> FileCategory {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "mkv" | "mp4" | "avi" | "mov" | "wmv" | "flv" | "m2ts" | "ts" | "m4v" | "webm"
        | "vob" | "mpg" | "mpeg" => FileCategory::Video,
        "mp3" | "flac" | "aac" | "ogg" | "wav" | "m4a" | "opus" | "wma" | "alac" | "ape" => {
            FileCategory::Audio
        }
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "zst" | "cbz" | "cbr" | "iso"
        | "tgz" | "tbz2" => FileCategory::Archive,
        "exe" | "msi" | "deb" | "rpm" | "appimage" | "pkg" | "dmg" | "flatpak" | "snap" => {
            FileCategory::Programs
        }
        _ => FileCategory::Other,
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
