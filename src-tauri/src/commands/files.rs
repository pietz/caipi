use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String, // "file" or "folder"
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileEntry>>,
}

/// Validates that a path is within the allowed root directory.
/// Returns the canonicalized path if valid, or an error if the path escapes the root.
fn validate_path_within_root(path: &str, root_path: &str) -> Result<PathBuf, String> {
    let requested = PathBuf::from(path);
    let root = PathBuf::from(root_path);

    // Canonicalize both paths to resolve symlinks and ../
    let canonical_root = root.canonicalize()
        .map_err(|e| format!("Failed to resolve root path: {}", e))?;
    let canonical_requested = requested.canonicalize()
        .map_err(|e| format!("Failed to resolve requested path: {}", e))?;

    // Check that the requested path starts with the root path
    if !canonical_requested.starts_with(&canonical_root) {
        return Err(format!(
            "Access denied: path '{}' is outside the project folder",
            path
        ));
    }

    Ok(canonical_requested)
}

#[tauri::command]
pub async fn list_directory(path: String, root_path: Option<String>) -> Result<Vec<FileEntry>, String> {
    // If root_path is provided, validate that the requested path is within it
    let dir_path = if let Some(root) = &root_path {
        validate_path_within_root(&path, root)?
    } else {
        PathBuf::from(&path)
    };

    if !dir_path.exists() {
        return Err("Directory does not exist".to_string());
    }

    if !dir_path.is_dir() {
        return Err("Path is not a directory".to_string());
    }

    let mut entries: Vec<FileEntry> = Vec::new();

    let read_dir = fs::read_dir(&dir_path).map_err(|e| e.to_string())?;

    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files and common ignore patterns
        if file_name.starts_with('.') {
            continue;
        }
        if file_name == "node_modules" || file_name == "target" || file_name == "__pycache__" {
            continue;
        }

        let file_path = entry.path();
        let is_dir = file_path.is_dir();

        let file_entry = FileEntry {
            name: file_name,
            entry_type: if is_dir { "folder".to_string() } else { "file".to_string() },
            path: file_path.to_string_lossy().to_string(),
            children: if is_dir { Some(Vec::new()) } else { None },
        };

        entries.push(file_entry);
    }

    // Sort: folders first, then files, alphabetically within each group
    entries.sort_by(|a, b| {
        match (&a.entry_type[..], &b.entry_type[..]) {
            ("folder", "file") => std::cmp::Ordering::Less,
            ("file", "folder") => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::{list_directory, validate_path_within_root};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validate_path_within_root_allows_subpath() {
        let root = tempdir().expect("tempdir");
        let child = root.path().join("child");
        fs::create_dir_all(&child).expect("create child");

        let result = validate_path_within_root(
            child.to_string_lossy().as_ref(),
            root.path().to_string_lossy().as_ref(),
        );

        assert!(result.is_ok());
    }

    #[test]
    fn validate_path_within_root_denies_outside_path() {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("tempdir");

        let result = validate_path_within_root(
            outside.path().to_string_lossy().as_ref(),
            root.path().to_string_lossy().as_ref(),
        );

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn validate_path_within_root_denies_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("tempdir");
        let link_path = root.path().join("link-outside");

        symlink(outside.path(), &link_path).expect("create symlink");

        let result = validate_path_within_root(
            link_path.to_string_lossy().as_ref(),
            root.path().to_string_lossy().as_ref(),
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn list_directory_filters_and_sorts() {
        let root = tempdir().expect("tempdir");
        let root_path = root.path();

        fs::create_dir_all(root_path.join("B-folder")).expect("create folder");
        fs::create_dir_all(root_path.join("a-folder")).expect("create folder");
        fs::create_dir_all(root_path.join("node_modules")).expect("create node_modules");
        fs::create_dir_all(root_path.join("target")).expect("create target");
        fs::create_dir_all(root_path.join("__pycache__")).expect("create __pycache__");
        fs::write(root_path.join("b.txt"), "b").expect("write file");
        fs::write(root_path.join("A.txt"), "a").expect("write file");
        fs::write(root_path.join(".hidden"), "hidden").expect("write hidden");

        let entries = list_directory(
            root_path.to_string_lossy().to_string(),
            Some(root_path.to_string_lossy().to_string()),
        )
        .await
        .expect("list_directory");

        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["a-folder", "B-folder", "A.txt", "b.txt"]);
    }

    #[tokio::test]
    async fn list_directory_rejects_non_directory() {
        let root = tempdir().expect("tempdir");
        let file_path = root.path().join("file.txt");
        fs::write(&file_path, "content").expect("write file");

        let result = list_directory(
            file_path.to_string_lossy().to_string(),
            Some(root.path().to_string_lossy().to_string()),
        )
        .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Path is not a directory");
    }

    #[tokio::test]
    async fn list_directory_rejects_outside_root() {
        let root = tempdir().expect("tempdir");
        let outside = tempdir().expect("tempdir");

        let result = list_directory(
            outside.path().to_string_lossy().to_string(),
            Some(root.path().to_string_lossy().to_string()),
        )
        .await;

        assert!(result.is_err());
    }
}
