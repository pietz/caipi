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

#[tauri::command]
pub async fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir_path = PathBuf::from(&path);

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
