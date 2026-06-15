use actix_web::{get, web, App, HttpResponse, HttpServer};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const WORKSPACE_DIR: &str = "./workspace";
const OUTPUT_DIR: &str = "./workspace/build";

#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
    content: String,
}

#[derive(Deserialize)]
struct SyncRequest {
    files: Vec<FileEntry>,
}

#[derive(Serialize)]
struct SyncResponse {
    success: bool,
    message: String,
}

#[derive(Serialize)]
struct CompileResponse {
    success: bool,
    logs: Vec<String>,
}

#[derive(Serialize)]
struct OutputResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    files: Option<Vec<FileEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logs: Option<Vec<String>>,
}

async fn sync_files(body: web::Json<SyncRequest>) -> HttpResponse {
    let engine = base64::engine::general_purpose::STANDARD;
    let workspace = PathBuf::from(WORKSPACE_DIR);

    // Clean workspace (except build dir)
    if workspace.exists() {
        let _ = fs::remove_dir_all(&workspace);
    }
    if let Err(e) = fs::create_dir_all(&workspace) {
        return HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            message: format!("Failed to create workspace: {}", e),
        });
    }

    let mut count = 0;
    for file in &body.files {
        let file_path = workspace.join(&file.path);
        if let Some(parent) = file_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return HttpResponse::InternalServerError().json(SyncResponse {
                    success: false,
                    message: format!("Failed to create dir for {}: {}", file.path, e),
                });
            }
        }
        match engine.decode(&file.content) {
            Ok(decoded) => {
                if let Err(e) = fs::write(&file_path, &decoded) {
                    return HttpResponse::InternalServerError().json(SyncResponse {
                        success: false,
                        message: format!("Failed to write {}: {}", file.path, e),
                    });
                }
                count += 1;
            }
            Err(e) => {
                return HttpResponse::BadRequest().json(SyncResponse {
                    success: false,
                    message: format!("Failed to decode {}: {}", file.path, e),
                });
            }
        }
    }

    HttpResponse::Ok().json(SyncResponse {
        success: true,
        message: format!("{} files synced", count),
    })
}

async fn compile() -> HttpResponse {
    let workspace = PathBuf::from(WORKSPACE_DIR);
    let workspace_abs = match fs::canonicalize(&workspace) {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError().json(CompileResponse {
                success: false,
                logs: vec![format!("Cannot resolve workspace path: {}", e)],
            });
        }
    };

    let workspace_str = workspace_abs.to_string_lossy().to_string();
    let workspace_str = workspace_str.strip_prefix(r"\\?\").unwrap_or(&workspace_str).to_string();

    // Run Docker container
    let result = Command::new("docker")
        .args([
            "run",
            "--rm",
            "-v",
            &format!("{}:/src", workspace_str),
            "build-container",
        ])
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut logs = Vec::new();
            if !stdout.is_empty() {
                logs.push(stdout);
            }
            if !stderr.is_empty() {
                logs.push(stderr);
            }

            HttpResponse::Ok().json(CompileResponse {
                success: output.status.success(),
                logs,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(CompileResponse {
            success: false,
            logs: vec![format!("Failed to run Docker: {}", e)],
        }),
    }
}

#[get("/output")]
async fn get_output() -> HttpResponse {
    let output_dir = PathBuf::from(OUTPUT_DIR);
    let engine = base64::engine::general_purpose::STANDARD;

    if !output_dir.exists() {
        return HttpResponse::Ok().json(OutputResponse {
            success: false,
            files: None,
            errors: Some(vec!["No build output directory found".into()]),
            logs: None,
        });
    }

    let mut files = Vec::new();
    collect_files(&output_dir, &output_dir, &engine, &mut files);

    if files.is_empty() {
        return HttpResponse::Ok().json(OutputResponse {
            success: false,
            files: None,
            errors: Some(vec!["Build produced no output files".into()]),
            logs: None,
        });
    }

    let count = files.len();
    HttpResponse::Ok().json(OutputResponse {
        success: true,
        files: Some(files),
        errors: None,
        logs: Some(vec![format!("{} artifact(s) ready", count)]),
    })
}

fn collect_files(
    dir: &PathBuf,
    base: &PathBuf,
    engine: &base64::engine::GeneralPurpose,
    files: &mut Vec<FileEntry>,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files(&path, base, engine, files);
            } else if path.is_file() {
                if let Ok(bytes) = fs::read(&path) {
                    let rel = path.strip_prefix(base)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    files.push(FileEntry {
                        path: rel,
                        content: engine.encode(&bytes),
                    });
                }
            }
        }
    }
}

fn get_dist_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("app/dist").exists() {
        cwd.join("app/dist")
    } else if cwd.join("../app/dist").exists() {
        cwd.join("../app/dist")
    } else {
        PathBuf::from("app/dist")
    }
}

async fn serve_static(path: web::Path<String>) -> HttpResponse {
    let dist_dir = get_dist_dir();
    let req_path = path.into_inner();
    
    let mut file_path = dist_dir.clone();
    if req_path.is_empty() || req_path == "/" {
        file_path.push("index.html");
    } else {
        file_path.push(&req_path);
    }
    
    // Fallback to index.html for SPA routing or if not found
    if !file_path.exists() || file_path.is_dir() {
        file_path = dist_dir.join("index.html");
    }
    
    if !file_path.exists() {
        return HttpResponse::NotFound().body("Static files not found in app/dist. Make sure to build the app (npm run build).");
    }
    
    match fs::read(&file_path) {
        Ok(content) => {
            let mime_type = match file_path.extension().and_then(|s| s.to_str()) {
                Some("html") => "text/html",
                Some("css") => "text/css",
                Some("js") => "application/javascript",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("ico") => "image/x-icon",
                Some("svg") => "image/svg+xml",
                _ => "application/octet-stream",
            };
            HttpResponse::Ok()
                .content_type(mime_type)
                .body(content)
        }
        Err(e) => HttpResponse::InternalServerError().body(format!("Failed to read static file: {}", e)),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Build server starting on http://localhost:3001");
    HttpServer::new(|| {
        App::new()
            .route("/sync", web::post().to(sync_files))
            .route("/compile", web::put().to(compile))
            .service(get_output)
            .route("/{filename:.*}", web::get().to(serve_static))
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await
}
