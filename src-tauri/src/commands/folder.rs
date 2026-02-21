use std::fs;
use std::path::PathBuf;

use crate::storage;

// Re-export from storage where the type now lives
pub use crate::storage::RecentFolder;

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

#[cfg(test)]
mod tests {
    use super::validate_folder;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn validate_folder_returns_true_for_directory() {
        let dir = tempdir().expect("tempdir");
        let result = validate_folder(dir.path().to_string_lossy().to_string())
            .await
            .expect("validate_folder");
        assert!(result);
    }

    #[tokio::test]
    async fn validate_folder_returns_false_for_missing_path() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing");

        let result = validate_folder(missing.to_string_lossy().to_string())
            .await
            .expect("validate_folder");
        assert!(!result);
    }

    #[tokio::test]
    async fn validate_folder_returns_false_for_file() {
        let dir = tempdir().expect("tempdir");
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "content").expect("write file");

        let result = validate_folder(file_path.to_string_lossy().to_string())
            .await
            .expect("validate_folder");
        assert!(!result);
    }
}
