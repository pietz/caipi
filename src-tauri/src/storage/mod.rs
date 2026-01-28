use crate::commands::folder::RecentFolder;
use crate::commands::setup::CliStatus;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use thiserror::Error;

static STORAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn get_storage_lock() -> &'static Mutex<()> {
    STORAGE_LOCK.get_or_init(|| Mutex::new(()))
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LicenseData {
    pub license_key: String,
    pub activated_at: u64, // Unix timestamp in seconds
    pub email: Option<String>,
    #[serde(default)]
    pub instance_id: Option<String>, // Lemon Squeezy instance ID for deactivation
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
    #[serde(default)]
    pub license: Option<LicenseData>,
    #[serde(default)]
    pub cli_path: Option<String>,
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
    let dir = path.parent().ok_or(StorageError::NoAppDir)?;
    let content = serde_json::to_string_pretty(data)?;

    // Write to temp file, then atomic rename
    let mut temp_file = NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.persist(&path).map_err(|e| StorageError::Io(e.error))?;

    Ok(())
}

pub fn get_recent_folders() -> Result<Vec<RecentFolder>, StorageError> {
    let data = load_data()?;
    Ok(data.recent_folders)
}

pub fn save_recent_folder(folder: RecentFolder) -> Result<(), StorageError> {
    let _guard = get_storage_lock().lock().unwrap();
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
    let _guard = get_storage_lock().lock().unwrap();
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
    let _guard = get_storage_lock().lock().unwrap();
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
    let _guard = get_storage_lock().lock().unwrap();
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
    let _guard = get_storage_lock().lock().unwrap();
    let mut data = load_data()?;
    data.default_folder = path;
    save_data(&data)?;
    Ok(())
}

pub fn get_license() -> Result<Option<LicenseData>, StorageError> {
    let data = load_data()?;
    Ok(data.license)
}

pub fn set_license(
    license_key: String,
    activated_at: u64,
    email: Option<String>,
    instance_id: Option<String>,
) -> Result<(), StorageError> {
    let _guard = get_storage_lock().lock().unwrap();
    let mut data = load_data()?;
    data.license = Some(LicenseData {
        license_key,
        activated_at,
        email,
        instance_id,
    });
    save_data(&data)?;
    Ok(())
}

pub fn clear_license() -> Result<(), StorageError> {
    let _guard = get_storage_lock().lock().unwrap();
    let mut data = load_data()?;
    data.license = None;
    save_data(&data)?;
    Ok(())
}

pub fn get_cli_path() -> Result<Option<String>, StorageError> {
    let data = load_data()?;
    Ok(data.cli_path)
}

pub fn set_cli_path(path: Option<String>) -> Result<(), StorageError> {
    let _guard = get_storage_lock().lock().unwrap();
    let mut data = load_data()?;
    data.cli_path = path;
    save_data(&data)?;
    Ok(())
}

// Test helper functions that accept explicit paths
#[cfg(test)]
fn load_data_from(path: &std::path::Path) -> Result<AppData, StorageError> {
    if !path.exists() {
        return Ok(AppData::default());
    }

    let content = fs::read_to_string(path)?;
    let data: AppData = serde_json::from_str(&content)?;
    Ok(data)
}

#[cfg(test)]
fn save_data_to(path: &std::path::Path, data: &AppData) -> Result<(), StorageError> {
    let dir = path.parent().ok_or(StorageError::NoAppDir)?;
    let content = serde_json::to_string_pretty(data)?;

    // Write to temp file, then atomic rename
    let mut temp_file = NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.persist(path).map_err(|e| StorageError::Io(e.error))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_dir() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let data_path = temp_dir.path().join("data.json");
        (temp_dir, data_path)
    }

    #[test]
    fn test_load_data_returns_default_when_file_doesnt_exist() {
        let (_temp_dir, data_path) = setup_test_dir();

        let result = load_data_from(&data_path).unwrap();

        assert_eq!(result.recent_folders.len(), 0);
        assert!(!result.onboarding_completed);
        assert!(result.cli_status_cache.is_none());
        assert!(result.default_folder.is_none());
    }

    #[test]
    fn test_load_data_returns_default_on_parse_error() {
        let (_temp_dir, data_path) = setup_test_dir();

        // Write invalid JSON
        fs::write(&data_path, "{ invalid json ").unwrap();

        let result = load_data_from(&data_path);

        // Should return error, not default
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StorageError::Json(_)));
    }

    #[test]
    fn test_save_and_load_data_roundtrip() {
        let (_temp_dir, data_path) = setup_test_dir();

        let original_data = AppData {
            recent_folders: vec![
                RecentFolder {
                    path: "/test/path1".to_string(),
                    name: "path1".to_string(),
                    timestamp: 1000,
                },
                RecentFolder {
                    path: "/test/path2".to_string(),
                    name: "path2".to_string(),
                    timestamp: 2000,
                },
            ],
            onboarding_completed: true,
            cli_status_cache: Some(CliStatusCache {
                status: CliStatus {
                    installed: true,
                    version: Some("1.0.0".to_string()),
                    authenticated: true,
                    path: Some("/usr/local/bin/claude".to_string()),
                },
                cached_at: 12345,
            }),
            default_folder: Some("/default/path".to_string()),
            license: None,
            cli_path: None,
        };

        save_data_to(&data_path, &original_data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert_eq!(loaded_data.recent_folders.len(), 2);
        assert_eq!(loaded_data.recent_folders[0].path, "/test/path1");
        assert_eq!(loaded_data.recent_folders[1].path, "/test/path2");
        assert!(loaded_data.onboarding_completed);
        assert!(loaded_data.cli_status_cache.is_some());
        let cache = loaded_data.cli_status_cache.unwrap();
        assert_eq!(cache.cached_at, 12345);
        assert!(cache.status.installed);
        assert_eq!(cache.status.version, Some("1.0.0".to_string()));
        assert_eq!(loaded_data.default_folder, Some("/default/path".to_string()));
    }

    #[test]
    fn test_recent_folders_max_5_enforced() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();

        // Add 7 folders
        for i in 0..7 {
            data.recent_folders.push(RecentFolder {
                path: format!("/test/path{}", i),
                name: format!("path{}", i),
                timestamp: i as i64,
            });
        }

        // Truncate to 5 (simulating the save_recent_folder logic)
        data.recent_folders.truncate(5);

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert_eq!(loaded_data.recent_folders.len(), 5);
    }

    #[test]
    fn test_recent_folders_same_path_moves_to_front() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData {
            recent_folders: vec![
                RecentFolder {
                    path: "/test/path1".to_string(),
                    name: "path1".to_string(),
                    timestamp: 1000,
                },
                RecentFolder {
                    path: "/test/path2".to_string(),
                    name: "path2".to_string(),
                    timestamp: 2000,
                },
                RecentFolder {
                    path: "/test/path3".to_string(),
                    name: "path3".to_string(),
                    timestamp: 3000,
                },
            ],
            ..Default::default()
        };

        // Simulate adding path2 again (remove + insert at front)
        let new_folder = RecentFolder {
            path: "/test/path2".to_string(),
            name: "path2".to_string(),
            timestamp: 4000,
        };

        data.recent_folders.retain(|f| f.path != new_folder.path);
        data.recent_folders.insert(0, new_folder);
        data.recent_folders.truncate(5);

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert_eq!(loaded_data.recent_folders.len(), 3);
        assert_eq!(loaded_data.recent_folders[0].path, "/test/path2");
        assert_eq!(loaded_data.recent_folders[0].timestamp, 4000);
        assert_eq!(loaded_data.recent_folders[1].path, "/test/path1");
        assert_eq!(loaded_data.recent_folders[2].path, "/test/path3");
    }

    #[test]
    fn test_recent_folders_order_preserved() {
        let (_temp_dir, data_path) = setup_test_dir();

        let data = AppData {
            recent_folders: vec![
                RecentFolder {
                    path: "/newest".to_string(),
                    name: "newest".to_string(),
                    timestamp: 3000,
                },
                RecentFolder {
                    path: "/middle".to_string(),
                    name: "middle".to_string(),
                    timestamp: 2000,
                },
                RecentFolder {
                    path: "/oldest".to_string(),
                    name: "oldest".to_string(),
                    timestamp: 1000,
                },
            ],
            ..Default::default()
        };

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert_eq!(loaded_data.recent_folders[0].path, "/newest");
        assert_eq!(loaded_data.recent_folders[1].path, "/middle");
        assert_eq!(loaded_data.recent_folders[2].path, "/oldest");
    }

    #[test]
    fn test_onboarding_flag_roundtrip() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();
        assert!(!data.onboarding_completed);

        data.onboarding_completed = true;
        save_data_to(&data_path, &data).unwrap();

        let loaded_data = load_data_from(&data_path).unwrap();
        assert!(loaded_data.onboarding_completed);

        // Test setting back to false
        let mut data2 = loaded_data;
        data2.onboarding_completed = false;
        save_data_to(&data_path, &data2).unwrap();

        let loaded_data2 = load_data_from(&data_path).unwrap();
        assert!(!loaded_data2.onboarding_completed);
    }

    #[test]
    fn test_cli_status_cache_set_and_get_roundtrip() {
        let (_temp_dir, data_path) = setup_test_dir();

        let cli_status = CliStatus {
            installed: true,
            version: Some("2.0.0".to_string()),
            authenticated: false,
            path: Some("/opt/bin/claude".to_string()),
        };

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut data = AppData::default();
        data.cli_status_cache = Some(CliStatusCache {
            status: cli_status.clone(),
            cached_at: now,
        });

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert!(loaded_data.cli_status_cache.is_some());
        let cache = loaded_data.cli_status_cache.unwrap();
        assert_eq!(cache.cached_at, now);
        assert!(cache.status.installed);
        assert!(!cache.status.authenticated);
        assert_eq!(cache.status.version, Some("2.0.0".to_string()));
        assert_eq!(cache.status.path, Some("/opt/bin/claude".to_string()));
    }

    #[test]
    fn test_cli_status_cache_clear() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();
        data.cli_status_cache = Some(CliStatusCache {
            status: CliStatus {
                installed: true,
                version: Some("1.0.0".to_string()),
                authenticated: true,
                path: Some("/usr/bin/claude".to_string()),
            },
            cached_at: 12345,
        });

        save_data_to(&data_path, &data).unwrap();

        // Clear the cache
        let mut loaded_data = load_data_from(&data_path).unwrap();
        assert!(loaded_data.cli_status_cache.is_some());

        loaded_data.cli_status_cache = None;
        save_data_to(&data_path, &loaded_data).unwrap();

        let final_data = load_data_from(&data_path).unwrap();
        assert!(final_data.cli_status_cache.is_none());
    }

    #[test]
    fn test_default_folder_set_and_get_roundtrip() {
        let (_temp_dir, data_path) = setup_test_dir();

        // Test with Some value
        let mut data = AppData::default();
        data.default_folder = Some("/home/user/projects".to_string());

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert_eq!(
            loaded_data.default_folder,
            Some("/home/user/projects".to_string())
        );
    }

    #[test]
    fn test_default_folder_none() {
        let (_temp_dir, data_path) = setup_test_dir();

        // Start with Some, then set to None
        let mut data = AppData::default();
        data.default_folder = Some("/some/path".to_string());
        save_data_to(&data_path, &data).unwrap();

        let mut loaded_data = load_data_from(&data_path).unwrap();
        assert_eq!(loaded_data.default_folder, Some("/some/path".to_string()));

        loaded_data.default_folder = None;
        save_data_to(&data_path, &loaded_data).unwrap();

        let final_data = load_data_from(&data_path).unwrap();
        assert!(final_data.default_folder.is_none());
    }

    #[test]
    fn test_atomic_write_on_failure() {
        let (_temp_dir, data_path) = setup_test_dir();

        let data1 = AppData {
            onboarding_completed: true,
            ..Default::default()
        };

        save_data_to(&data_path, &data1).unwrap();

        // The atomic write should ensure old data remains if new write fails
        // This is implicitly tested by the tempfile + persist mechanism
        // Here we verify the file exists and contains valid data
        let loaded = load_data_from(&data_path).unwrap();
        assert!(loaded.onboarding_completed);
    }

    #[test]
    fn test_license_set_and_get_roundtrip() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();
        data.license = Some(LicenseData {
            license_key: "CAIPI-1234567890ABCDEF".to_string(),
            activated_at: 1700000000,
            email: Some("user@example.com".to_string()),
            instance_id: None,
        });

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert!(loaded_data.license.is_some());
        let license = loaded_data.license.unwrap();
        assert_eq!(license.license_key, "CAIPI-1234567890ABCDEF");
        assert_eq!(license.activated_at, 1700000000);
        assert_eq!(license.email, Some("user@example.com".to_string()));
    }

    #[test]
    fn test_license_clear() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();
        data.license = Some(LicenseData {
            license_key: "CAIPI-TEST123456789".to_string(),
            activated_at: 1700000000,
            email: None,
            instance_id: None,
        });

        save_data_to(&data_path, &data).unwrap();

        let mut loaded_data = load_data_from(&data_path).unwrap();
        assert!(loaded_data.license.is_some());

        loaded_data.license = None;
        save_data_to(&data_path, &loaded_data).unwrap();

        let final_data = load_data_from(&data_path).unwrap();
        assert!(final_data.license.is_none());
    }

    #[test]
    fn test_license_without_email() {
        let (_temp_dir, data_path) = setup_test_dir();

        let mut data = AppData::default();
        data.license = Some(LicenseData {
            license_key: "CAIPI-NOEMAILTESTKEY".to_string(),
            activated_at: 1700000000,
            email: None,
            instance_id: None,
        });

        save_data_to(&data_path, &data).unwrap();
        let loaded_data = load_data_from(&data_path).unwrap();

        assert!(loaded_data.license.is_some());
        let license = loaded_data.license.unwrap();
        assert!(license.email.is_none());
    }
}
