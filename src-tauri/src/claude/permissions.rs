#![allow(dead_code)]
use std::collections::HashMap;

/// Translates technical tool operations into human-readable descriptions
pub fn translate_permission(tool: &str, input: &serde_json::Value) -> String {
    match tool {
        "Write" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("a file");
            format!("Create or overwrite the file '{}'", path)
        }
        "Edit" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("a file");
            format!("Edit the file '{}'", path)
        }
        "Bash" => {
            let command = input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("a command");
            translate_bash_command(command)
        }
        "Read" => {
            let path = input
                .get("file_path")
                .and_then(|v| v.as_str())
                .unwrap_or("a file");
            format!("Read the file '{}'", path)
        }
        "Glob" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("*");
            format!("Search for files matching '{}'", pattern)
        }
        "Grep" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("...");
            format!("Search file contents for '{}'", pattern)
        }
        _ => format!("Execute {} operation", tool),
    }
}

fn translate_bash_command(command: &str) -> String {
    let cmd = command.trim();
    let parts: Vec<&str> = cmd.split_whitespace().collect();

    if parts.is_empty() {
        return "Run a shell command".to_string();
    }

    let base = parts[0];

    // Common command translations
    let translations: HashMap<&str, fn(&[&str]) -> String> = {
        let mut m: HashMap<&str, fn(&[&str]) -> String> = HashMap::new();

        m.insert("rm", |p| {
            if p.contains(&"-rf") || p.contains(&"-r") {
                let target = p.last().unwrap_or(&"items");
                format!("Delete '{}' and all its contents", target)
            } else {
                let target = p.last().unwrap_or(&"file");
                format!("Delete the file '{}'", target)
            }
        });

        m.insert("mkdir", |p| {
            let dir = p.last().unwrap_or(&"directory");
            format!("Create the directory '{}'", dir)
        });

        m.insert("mv", |p| {
            if p.len() >= 3 {
                format!("Move '{}' to '{}'", p[p.len() - 2], p[p.len() - 1])
            } else {
                "Move files".to_string()
            }
        });

        m.insert("cp", |p| {
            if p.contains(&"-r") || p.contains(&"-R") {
                "Copy files and directories".to_string()
            } else {
                "Copy files".to_string()
            }
        });

        m.insert("npm", |p| {
            if p.contains(&"install") || p.contains(&"i") {
                "Install npm packages".to_string()
            } else if p.contains(&"run") {
                let script = p.iter().skip_while(|&&x| x != "run").nth(1).unwrap_or(&"script");
                format!("Run npm script '{}'", script)
            } else {
                "Run npm command".to_string()
            }
        });

        m.insert("git", |p| {
            if p.contains(&"commit") {
                "Create a git commit".to_string()
            } else if p.contains(&"push") {
                "Push changes to remote".to_string()
            } else if p.contains(&"pull") {
                "Pull changes from remote".to_string()
            } else if p.contains(&"checkout") {
                "Switch git branch".to_string()
            } else {
                "Run git command".to_string()
            }
        });

        m.insert("cargo", |p| {
            if p.contains(&"build") {
                "Build the Rust project".to_string()
            } else if p.contains(&"run") {
                "Run the Rust project".to_string()
            } else if p.contains(&"test") {
                "Run tests".to_string()
            } else {
                "Run cargo command".to_string()
            }
        });

        m.insert("python", |_| "Run Python script".to_string());
        m.insert("python3", |_| "Run Python script".to_string());
        m.insert("node", |_| "Run Node.js script".to_string());

        m
    };

    if let Some(translator) = translations.get(base) {
        translator(&parts)
    } else {
        // For unknown commands, show a shortened version
        if cmd.len() > 60 {
            format!("Run command: {}...", &cmd[..57])
        } else {
            format!("Run command: {}", cmd)
        }
    }
}
