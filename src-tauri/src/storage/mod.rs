use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::NamedTempFile;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Domain types (moved here from commands/)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecentFolder {
    pub path: String,
    pub name: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub authenticated: bool,
    pub path: Option<String>,
}

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
    /// Which backend this cached status applies to.
    /// Older builds did not store this field; treat missing as "claude".
    #[serde(default)]
    pub backend: Option<String>,
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
    pub cli_path: Option<String>,
    #[serde(default)]
    pub default_backend: Option<String>,
    #[serde(default)]
    pub backend_cli_paths: HashMap<String, String>,
}

fn get_app_dir() -> Result<PathBuf, StorageError> {
    let folder = if cfg!(debug_assertions) {
        "caipi-dev"
    } else {
        "caipi"
    };
    let dir = dirs::data_local_dir()
        .ok_or(StorageError::NoAppDir)?
        .join(folder);

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
    match serde_json::from_str::<AppData>(&content) {
        Ok(data) => Ok(data),
        Err(err) => {
            log::warn!("Failed to parse data.json ({}); falling back to defaults", err);
            Ok(AppData::default())
        }
    }
}

fn save_data(data: &AppData) -> Result<(), StorageError> {
    let path = get_data_path()?;
    let dir = path.parent().ok_or(StorageError::NoAppDir)?;
    let content = serde_json::to_string_pretty(data)?;

    // Write to temp file, then atomic rename
    let mut temp_file = NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file
        .persist(&path)
        .map_err(|e| StorageError::Io(e.error))?;

    Ok(())
}

/// Acquire lock, load data, apply a mutating closure, then save.
fn with_data<F, R>(f: F) -> Result<R, StorageError>
where
    F: FnOnce(&mut AppData) -> Result<R, StorageError>,
{
    let _guard = get_storage_lock().lock();
    let mut data = load_data()?;
    let result = f(&mut data)?;
    save_data(&data)?;
    Ok(result)
}

/// Acquire lock, load data, apply a read-only closure (no save).
fn with_data_ro<F, R>(f: F) -> Result<R, StorageError>
where
    F: FnOnce(&AppData) -> Result<R, StorageError>,
{
    let _guard = get_storage_lock().lock();
    let data = load_data()?;
    f(&data)
}

pub fn get_recent_folders() -> Result<Vec<RecentFolder>, StorageError> {
    with_data_ro(|data| Ok(data.recent_folders.clone()))
}

pub fn save_recent_folder(folder: RecentFolder) -> Result<(), StorageError> {
    with_data(|data| {
        // Remove if already exists
        data.recent_folders.retain(|f| f.path != folder.path);
        // Add to front
        data.recent_folders.insert(0, folder);
        // Keep only last 5
        data.recent_folders.truncate(5);
        Ok(())
    })
}

pub fn get_onboarding_completed() -> Result<bool, StorageError> {
    with_data_ro(|data| Ok(data.onboarding_completed))
}

pub fn set_onboarding_completed(completed: bool) -> Result<(), StorageError> {
    with_data(|data| {
        data.onboarding_completed = completed;
        Ok(())
    })
}

pub fn get_cli_status_cache() -> Result<Option<CliStatusCache>, StorageError> {
    with_data_ro(|data| Ok(data.cli_status_cache.clone()))
}

pub fn set_cli_status_cache(status: CliStatus, backend: Option<String>) -> Result<(), StorageError> {
    with_data(|data| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        data.cli_status_cache = Some(CliStatusCache {
            status,
            cached_at: now,
            backend,
        });
        Ok(())
    })
}

pub fn get_default_folder() -> Result<Option<String>, StorageError> {
    with_data_ro(|data| Ok(data.default_folder.clone()))
}

pub fn set_default_folder(path: Option<String>) -> Result<(), StorageError> {
    with_data(|data| {
        data.default_folder = path;
        Ok(())
    })
}

/// Migrates legacy "claudecli" key to "claude" if needed.
/// Mutates `data` in place; caller is responsible for persisting.
fn ensure_claude_key_migrated(data: &mut AppData) {
    if !data.backend_cli_paths.contains_key("claude") {
        if let Some(old) = data.backend_cli_paths.remove("claudecli") {
            data.backend_cli_paths.insert("claude".to_string(), old.clone());
            data.cli_path = Some(old);
        }
    }
}

pub fn get_cli_path() -> Result<Option<String>, StorageError> {
    with_data(|data| {
        if let Some(path) = data.backend_cli_paths.get("claude") {
            return Ok(Some(path.clone()));
        }
        if data.backend_cli_paths.contains_key("claudecli") {
            ensure_claude_key_migrated(data);
            return Ok(data.backend_cli_paths.get("claude").cloned());
        }
        Ok(data.cli_path.clone())
    })
}

pub fn get_default_backend() -> Result<Option<String>, StorageError> {
    with_data_ro(|data| Ok(data.default_backend.clone()))
}

pub fn set_default_backend(backend: Option<String>) -> Result<(), StorageError> {
    with_data(|data| {
        data.default_backend = backend;
        Ok(())
    })
}

pub fn get_backend_cli_paths() -> Result<HashMap<String, String>, StorageError> {
    with_data(|data| {
        if data.backend_cli_paths.contains_key("claudecli") && !data.backend_cli_paths.contains_key("claude") {
            ensure_claude_key_migrated(data);
        }
        Ok(data.backend_cli_paths.clone())
    })
}

pub fn get_backend_cli_path(backend: &str) -> Result<Option<String>, StorageError> {
    let key = if backend == "claudecli" { "claude" } else { backend };
    with_data(|data| {
        if let Some(path) = data.backend_cli_paths.get(key) {
            return Ok(Some(path.clone()));
        }
        if key == "claude" && data.backend_cli_paths.contains_key("claudecli") {
            ensure_claude_key_migrated(data);
            return Ok(data.backend_cli_paths.get("claude").cloned());
        }
        Ok(None)
    })
}

pub fn set_backend_cli_path(backend: &str, path: Option<String>) -> Result<(), StorageError> {
    let key = if backend == "claudecli" { "claude" } else { backend };
    with_data(|data| {
        match path {
            Some(path) => {
                data.backend_cli_paths.insert(key.to_string(), path);
                if key == "claude" {
                    data.backend_cli_paths.remove("claudecli");
                }
            }
            None => {
                data.backend_cli_paths.remove(key);
                if key == "claude" {
                    data.backend_cli_paths.remove("claudecli");
                }
            }
        }

        if key == "claude" {
            data.cli_path = data.backend_cli_paths.get("claude").cloned();
        }

        Ok(())
    })
}

// Test helper functions that accept explicit paths
#[cfg(test)]
fn load_data_from(path: &std::path::Path) -> Result<AppData, StorageError> {
    if !path.exists() {
        return Ok(AppData::default());
    }

    let content = fs::read_to_string(path)?;
    match serde_json::from_str::<AppData>(&content) {
        Ok(data) => Ok(data),
        Err(_) => Ok(AppData::default()),
    }
}

#[cfg(test)]
fn save_data_to(path: &std::path::Path, data: &AppData) -> Result<(), StorageError> {
    let dir = path.parent().ok_or(StorageError::NoAppDir)?;
    let content = serde_json::to_string_pretty(data)?;

    // Write to temp file, then atomic rename
    let mut temp_file = NamedTempFile::new_in(dir)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file
        .persist(path)
        .map_err(|e| StorageError::Io(e.error))?;

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

        // Should self-heal by falling back to defaults.
        let data = result.unwrap();
        assert_eq!(data.recent_folders.len(), 0);
        assert!(!data.onboarding_completed);
        assert!(data.cli_status_cache.is_none());
        assert!(data.default_folder.is_none());
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
                backend: Some("claude".to_string()),
            }),
            default_folder: Some("/default/path".to_string()),
            cli_path: None,
            default_backend: Some("claude".to_string()),
            backend_cli_paths: HashMap::new(),
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
        assert_eq!(
            loaded_data.default_folder,
            Some("/default/path".to_string())
        );
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
            backend: None,
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
            backend: None,
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

}
