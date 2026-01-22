use crate::commands::folder::RecentFolder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
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

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AppData {
    pub recent_folders: Vec<RecentFolder>,
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
