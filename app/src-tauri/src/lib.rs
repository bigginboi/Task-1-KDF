use tauri::command;
use std::fs;
use std::io::{Write, Cursor};
use std::path::Path;
use base64::{engine::general_purpose, Engine as _};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipWriter, ZipArchive};

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
fn zip_folder(path: String) -> Result<String, String> {
    let base = Path::new(&path);
    if !base.is_dir() {
        return Err("not a directory".into());
    }

    let buf: Vec<u8> = Vec::new();
    let cursor = Cursor::new(buf);
    let mut zip = ZipWriter::new(cursor);
    let opts = SimpleFileOptions::default();

    let skip = ["target", "build", ".git", "node_modules", "dist"];

    for entry in WalkDir::new(base).into_iter().filter_map(|e| e.ok()) {
        let full = entry.path();
        let rel = match full.strip_prefix(base) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        if rel_str.is_empty() {
            continue;
        }
        if skip.iter().any(|s| rel_str.starts_with(s)) {
            continue;
        }

        if full.is_file() {
            zip.start_file(&rel_str, opts).map_err(|e| e.to_string())?;
            let data = fs::read(full).map_err(|e| e.to_string())?;
            zip.write_all(&data).map_err(|e| e.to_string())?;
        }
    }

    let cursor = zip.finish().map_err(|e| e.to_string())?;
    let bytes = cursor.into_inner();
    Ok(general_purpose::STANDARD.encode(&bytes))
}

#[command]
fn extract_zip(data: String, dest_path: String) -> Result<Vec<String>, String> {
    let bytes = general_purpose::STANDARD.decode(&data).map_err(|e| e.to_string())?;
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| e.to_string())?;
    let dest = Path::new(&dest_path);
    let mut extracted = Vec::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().to_string();
        let outpath = dest.join(&name);

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![read_dir, zip_folder, extract_zip])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
