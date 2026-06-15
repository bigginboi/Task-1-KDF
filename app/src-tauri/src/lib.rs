use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

const BACKEND_URL: &str = "http://localhost:3001";

#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct SyncRequest {
    files: Vec<FileEntry>,
}

#[derive(Serialize, Deserialize)]
struct SyncResponse {
    success: bool,
    message: String,
}

#[derive(Serialize, Deserialize)]
struct CompileResponse {
    success: bool,
    logs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct OutputResponse {
    success: bool,
    files: Option<Vec<FileEntry>>,
    errors: Option<Vec<String>>,
    logs: Option<Vec<String>>,
}

#[derive(Serialize)]
struct BuildResult {
    success: bool,
    message: String,
}

fn emit_log(app: &AppHandle, msg: &str) {
    let _ = app.emit("build-log", msg.to_string());
}

fn emit_status(app: &AppHandle, msg: &str) {
    let _ = app.emit("build-status", msg.to_string());
}

#[tauri::command]
async fn sync_and_compile(app: AppHandle, folder_path: String) -> Result<BuildResult, String> {
    let base_path = PathBuf::from(&folder_path);
    if !base_path.exists() || !base_path.is_dir() {
        return Err("Selected folder does not exist".into());
    }

    let client = reqwest::Client::new();
    let engine = base64::engine::general_purpose::STANDARD;

    // Step 1: Read and sync files
    emit_status(&app, "Syncing Files...");
    emit_log(&app, "Reading local files...");

    let mut files: Vec<FileEntry> = Vec::new();
    for entry in WalkDir::new(&base_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let full_path = entry.path();
            let rel_path = full_path.strip_prefix(&base_path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");

            match fs::read(full_path) {
                Ok(bytes) => {
                    let encoded = engine.encode(&bytes);
                    files.push(FileEntry { path: rel_path.clone(), content: encoded });
                    emit_log(&app, &format!("  {}", rel_path));
                }
                Err(e) => {
                    emit_log(&app, &format!("  SKIP {}: {}", rel_path, e));
                }
            }
        }
    }

    emit_log(&app, &format!("Found {} files", files.len()));

    if files.is_empty() {
        return Ok(BuildResult {
            success: false,
            message: "No files found in selected folder".into(),
        });
    }

    // POST /sync
    emit_log(&app, "\nSyncing to server...");
    let sync_resp = client
        .post(format!("{}/sync", BACKEND_URL))
        .json(&SyncRequest { files })
        .send()
        .await
        .map_err(|e| format!("Sync request failed: {}", e))?;

    let sync_result: SyncResponse = sync_resp.json().await
        .map_err(|e| format!("Failed to parse sync response: {}", e))?;

    if !sync_result.success {
        return Ok(BuildResult {
            success: false,
            message: format!("Sync failed: {}", sync_result.message),
        });
    }
    emit_log(&app, &format!("Sync: {}", sync_result.message));

    // PUT /compile
    emit_status(&app, "Starting Compile...");
    emit_log(&app, "\nStarting compile...");

    let compile_resp = client
        .put(format!("{}/compile", BACKEND_URL))
        .send()
        .await
        .map_err(|e| format!("Compile request failed: {}", e))?;

    let compile_result: CompileResponse = compile_resp.json().await
        .map_err(|e| format!("Failed to parse compile response: {}", e))?;

    for log_line in &compile_result.logs {
        emit_log(&app, log_line);
    }

    if !compile_result.success {
        return Ok(BuildResult {
            success: false,
            message: "Compilation failed".into(),
        });
    }

    // GET /output
    emit_status(&app, "Retrieving Output...");
    emit_log(&app, "\nRetrieving build output...");

    let output_resp = client
        .get(format!("{}/output", BACKEND_URL))
        .send()
        .await
        .map_err(|e| format!("Output request failed: {}", e))?;

    let output_result: OutputResponse = output_resp.json().await
        .map_err(|e| format!("Failed to parse output response: {}", e))?;

    if let Some(logs) = &output_result.logs {
        for log_line in logs {
            emit_log(&app, log_line);
        }
    }

    if !output_result.success {
        if let Some(errors) = &output_result.errors {
            for err in errors {
                emit_log(&app, &format!("ERROR: {}", err));
            }
        }
        return Ok(BuildResult {
            success: false,
            message: "Build failed — see output for details".into(),
        });
    }

    // Save artifacts locally
    if let Some(output_files) = &output_result.files {
        let output_dir = base_path.join("build-output");
        for file in output_files {
            let file_path = output_dir.join(&file.path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
            }
            let decoded = engine.decode(&file.content)
                .map_err(|e| format!("Failed to decode file {}: {}", file.path, e))?;
            fs::write(&file_path, &decoded)
                .map_err(|e| format!("Failed to write file {}: {}", file.path, e))?;
            emit_log(&app, &format!("Saved: build-output/{}", file.path));
        }
        emit_log(&app, &format!("\n{} artifact(s) saved to build-output/", output_files.len()));
    }

    Ok(BuildResult {
        success: true,
        message: "Build completed successfully".into(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![sync_and_compile])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
