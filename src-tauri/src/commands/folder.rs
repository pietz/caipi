use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::storage;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecentFolder {
    pub path: String,
    pub name: String,
    pub timestamp: i64,
}

#[tauri::command]
pub async fn validate_folder(path: String) -> Result<bool, String> {
    let path = PathBuf::from(&path);

    if !path.exists() {
        return Ok(false);
    }

    if !path.is_dir() {
        return Ok(false);
    }

    // Check if readable
    match fs::read_dir(&path) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
pub async fn get_recent_folders() -> Result<Vec<RecentFolder>, String> {
    storage::get_recent_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_recent_folder(path: String) -> Result<(), String> {
    let path_buf = PathBuf::from(&path);
    let name = path_buf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let folder = RecentFolder {
        path,
        name,
        timestamp: chrono::Utc::now().timestamp(),
    };

    storage::save_recent_folder(folder).map_err(|e| e.to_string())
}
