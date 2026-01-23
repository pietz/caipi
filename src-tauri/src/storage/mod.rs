use crate::commands::folder::RecentFolder;
use crate::commands::setup::CliStatus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Could not find app data directory")]
    NoAppDir,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CliStatusCache {
    pub status: CliStatus,
    pub cached_at: u64, // Unix timestamp in seconds
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppData {
    pub recent_folders: Vec<RecentFolder>,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub cli_status_cache: Option<CliStatusCache>,
    #[serde(default)]
    pub default_folder: Option<String>,
}

fn get_app_dir() -> Result<PathBuf, StorageError> {
    let dir = dirs::data_local_dir()
        .ok_or(StorageError::NoAppDir)?
        .join("caipi");

    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    Ok(dir)
}

fn get_data_path() -> Result<PathBuf, StorageError> {
    Ok(get_app_dir()?.join("data.json"))
}

fn load_data() -> Result<AppData, StorageError> {
    let path = get_data_path()?;

    if !path.exists() {
        return Ok(AppData::default());
    }

    let content = fs::read_to_string(path)?;
    let data: AppData = serde_json::from_str(&content)?;
    Ok(data)
}

fn save_data(data: &AppData) -> Result<(), StorageError> {
    let path = get_data_path()?;
    let content = serde_json::to_string_pretty(data)?;
    fs::write(path, content)?;
    Ok(())
}

pub fn get_recent_folders() -> Result<Vec<RecentFolder>, StorageError> {
    let data = load_data()?;
    Ok(data.recent_folders)
}

pub fn save_recent_folder(folder: RecentFolder) -> Result<(), StorageError> {
    let mut data = load_data()?;

    // Remove if already exists
    data.recent_folders.retain(|f| f.path != folder.path);

    // Add to front
    data.recent_folders.insert(0, folder);

    // Keep only last 5
    data.recent_folders.truncate(5);

    save_data(&data)?;
    Ok(())
}

pub fn get_onboarding_completed() -> Result<bool, StorageError> {
    let data = load_data()?;
    Ok(data.onboarding_completed)
}

pub fn set_onboarding_completed(completed: bool) -> Result<(), StorageError> {
    let mut data = load_data()?;
    data.onboarding_completed = completed;
    save_data(&data)?;
    Ok(())
}

pub fn get_cli_status_cache() -> Result<Option<CliStatusCache>, StorageError> {
    let data = load_data()?;
    Ok(data.cli_status_cache)
}

pub fn set_cli_status_cache(status: CliStatus) -> Result<(), StorageError> {
    let mut data = load_data()?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    data.cli_status_cache = Some(CliStatusCache {
        status,
        cached_at: now,
    });
    save_data(&data)?;
    Ok(())
}

pub fn clear_cli_status_cache() -> Result<(), StorageError> {
    let mut data = load_data()?;
    data.cli_status_cache = None;
    save_data(&data)?;
    Ok(())
}

pub fn get_default_folder() -> Result<Option<String>, StorageError> {
    let data = load_data()?;
    Ok(data.default_folder)
}

pub fn set_default_folder(path: Option<String>) -> Result<(), StorageError> {
    let mut data = load_data()?;
    data.default_folder = path;
    save_data(&data)?;
    Ok(())
}
