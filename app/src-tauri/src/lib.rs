use tauri::command;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use base64::{engine::general_purpose, Engine as _};
use walkdir::WalkDir;
use zip::ZipArchive;

#[command]
fn read_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    let reader = fs::read_dir(&path).map_err(|e| format!("read dir: {}", e))?;

    for item in reader.flatten() {
        let is_dir = item.metadata().map(|m| m.is_dir()).unwrap_or(false);
        entries.push(FileEntry {
            name: item.file_name().to_string_lossy().to_string(),
            is_dir,
        });
    }

    Ok(entries)
}

#[command]
fn get_project_files(path: String) -> Result<Vec<ProjectFileEntry>, String> {
    let base = Path::new(&path);
    if !base.is_dir() {
        return Err("not a directory".into());
    }

    let mut files = Vec::new();
    let skip = ["target", "build", ".git", "node_modules", "dist", "__pycache__"];

    for entry in WalkDir::new(base).into_iter().filter_map(|e| e.ok()) {
        let full = entry.path();
        let rel = match full.strip_prefix(base) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let has_skipped_component = rel.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            s.starts_with('.') || skip.contains(&s.as_ref())
        });
        if has_skipped_component {
            continue;
        }

        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.is_empty() {
            continue;
        }

        if full.is_file() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.file_type().is_symlink() {
                    continue;
                }
            }

            match fs::read(full) {
                Ok(data) => {
                    let content = general_purpose::STANDARD.encode(&data);
                    files.push(ProjectFileEntry {
                        path: rel_str,
                        content,
                    });
                }
                Err(e) => {
                    eprintln!("Failed to read file {}: {}", rel_str, e);
                }
            }
        }
    }

    Ok(files)
}

#[command]
fn extract_zip(data: String, dest_path: String) -> Result<Vec<String>, String> {
    let bytes = general_purpose::STANDARD.decode(&data).map_err(|e| e.to_string())?;
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let dest = Path::new(&dest_path);
    let mut extracted = Vec::new();

    let output_dir = dest.join("build");
    fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        let outpath = output_dir.join(&name);

        if file.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
            extracted.push(name);
        }
    }

    Ok(extracted)
}

#[derive(serde::Serialize)]
struct FileEntry {
    name: String,
    is_dir: bool,
}

#[derive(serde::Serialize)]
struct ProjectFileEntry {
    path: String,
    content: String,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_dir, get_project_files, extract_zip])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
