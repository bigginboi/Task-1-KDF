use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::collections::HashMap;
use std::time::{Instant, Duration};

use crate::docker;

pub struct RateLimiter {
    pub requests: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
        }
    }

    pub fn check_rate_limit(&self, ip: String) -> bool {
        let mut reqs = match self.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let now = Instant::now();
        
        // Trim expired requests and remove empty IPs from the map to prevent key leaks
        reqs.retain(|_, times| {
            times.retain(|&t| now.duration_since(t) < Duration::from_secs(60));
            !times.is_empty()
        });

        let times = reqs.entry(ip).or_insert_with(Vec::new);
        if times.len() >= 10 {
            false
        } else {
            times.push(now);
            true
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error_code: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct SyncQuery {
    pub source_type: String,
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub success: bool,
    pub workspace_id: String,
    pub source_type: String,
}

#[derive(Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct SyncRequest {
    pub files: Vec<FileEntry>,
}

#[derive(Deserialize)]
pub struct CompileQuery {
    pub workspace_id: String,
    pub source_type: String,
}

#[derive(Serialize)]
pub struct CompileResponse {
    pub success: bool,
    pub workspace_id: String,
    pub status_code: i32,
    pub output: String,
}

#[derive(Deserialize)]
pub struct OutputQuery {
    pub workspace_id: String,
}

fn is_path_safe(path_str: &str) -> bool {
    if path_str.is_empty() {
        return false;
    }
    // Reject slashes, directory traversal, and backslashes (since client standardizes paths to forward slashes)
    if path_str.contains("..") || path_str.starts_with('/') || path_str.contains('\\') {
        return false;
    }
    for c in path_str.chars() {
        if !c.is_alphanumeric() && c != '/' && c != '.' && c != '_' && c != '-' {
            return false;
        }
    }
    true
}

fn is_valid_uuid(id: &str) -> bool {
    Uuid::parse_str(id).is_ok()
}

pub fn verify_rate_limit(req: &actix_web::HttpRequest, limiter: &RateLimiter) -> Result<(), HttpResponse> {
    let ip = req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    if !limiter.check_rate_limit(ip) {
        Err(HttpResponse::build(actix_web::http::StatusCode::TOO_MANY_REQUESTS).json(ErrorResponse {
            success: false,
            error_code: "RATE_LIMIT_EXCEEDED".to_string(),
            message: "Rate limit exceeded. Max 10 requests per minute.".to_string(),
        }))
    } else {
        Ok(())
    }
}

pub async fn sync_handler(
    req: actix_web::HttpRequest,
    body: web::Json<SyncRequest>,
    query: web::Query<SyncQuery>,
    limiter: web::Data<RateLimiter>,
) -> HttpResponse {
    if let Err(resp) = verify_rate_limit(&req, &limiter) {
        return resp;
    }

    if body.files.is_empty() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error_code: "FILE_WRITE_FAILED".to_string(),
            message: "At least one file is required".to_string(),
        });
    }

    let source_type = &query.source_type;

    if !["c", "cpp", "rust"].contains(&source_type.as_str()) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error_code: "INVALID_SOURCE_TYPE".to_string(),
            message: "Source type must be c, cpp, or rust".to_string(),
        });
    }

    if let Err(e) = docker::ensure_running() {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            success: false,
            error_code: "DOCKER_UNAVAILABLE".to_string(),
            message: format!("Docker unavailable: {}", e),
        });
    }

    for file in &body.files {
        if !is_path_safe(&file.path) {
            return HttpResponse::Forbidden().json(ErrorResponse {
                success: false,
                error_code: "INVALID_PATH".to_string(),
                message: format!("Invalid or unsafe file path: {}", file.path),
            });
        }

        let decoded_len = file.content.len() * 3 / 4;
        if decoded_len > 50 * 1024 * 1024 {
            return HttpResponse::build(actix_web::http::StatusCode::INSUFFICIENT_STORAGE).json(ErrorResponse {
                success: false,
                error_code: "PAYLOAD_TOO_LARGE".to_string(),
                message: format!("File exceeds 50MB limit: {}", file.path),
            });
        }
    }

    let uuid = Uuid::new_v4().to_string();
    let workspace_dir = PathBuf::from("./workspace").join(&uuid);

    if let Err(e) = fs::create_dir_all(&workspace_dir) {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error_code: "FILE_WRITE_FAILED".to_string(),
            message: format!("Failed to create workspace directory: {}", e),
        });
    }

    for file in &body.files {
        let file_path = workspace_dir.join(&file.path);
        
        if let Some(parent) = file_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return HttpResponse::InternalServerError().json(ErrorResponse {
                    success: false,
                    error_code: "FILE_WRITE_FAILED".to_string(),
                    message: format!("Failed to create folder structure: {}", e),
                });
            }
        }

        let decoded = match general_purpose::STANDARD.decode(&file.content) {
            Ok(bytes) => bytes,
            Err(e) => {
                return HttpResponse::BadRequest().json(ErrorResponse {
                    success: false,
                    error_code: "FILE_WRITE_FAILED".to_string(),
                    message: format!("Invalid base64 in file {}: {}", file.path, e),
                });
            }
        };

        if let Err(e) = fs::write(&file_path, &decoded) {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                success: false,
                error_code: "FILE_WRITE_FAILED".to_string(),
                message: format!("Failed to write file {}: {}", file.path, e),
            });
        }
    }

    if let Err(e) = docker::copy_workspace(&uuid) {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error_code: "DOCKER_EXEC_FAILED".to_string(),
            message: format!("Failed to copy workspace to Docker: {}", e),
        });
    }

    HttpResponse::Ok().json(SyncResponse {
        success: true,
        workspace_id: uuid,
        source_type: source_type.clone(),
    })
}

pub async fn compile_handler(
    req: actix_web::HttpRequest,
    query: web::Query<CompileQuery>,
    limiter: web::Data<RateLimiter>,
) -> HttpResponse {
    if let Err(resp) = verify_rate_limit(&req, &limiter) {
        return resp;
    }

    let workspace_id = &query.workspace_id;

    if !is_valid_uuid(workspace_id) {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    if !["c", "cpp", "rust"].contains(&query.source_type.as_str()) {
        return HttpResponse::BadRequest().json(ErrorResponse {
            success: false,
            error_code: "INVALID_SOURCE_TYPE".to_string(),
            message: "Source type must be c, cpp, or rust".to_string(),
        });
    }

    let workspace_dir = PathBuf::from("./workspace").join(workspace_id);

    if !workspace_dir.exists() {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    if let Err(e) = docker::ensure_running() {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            success: false,
            error_code: "DOCKER_UNAVAILABLE".to_string(),
            message: format!("Docker unavailable: {}", e),
        });
    }

    // Touch the workspace directory to reset the expiration timer
    let _ = fs::write(workspace_dir.join(".active"), b"");

    let (success, code, output) = docker::exec_compile(workspace_id, &query.source_type);

    HttpResponse::Ok().json(CompileResponse {
        success,
        workspace_id: workspace_id.clone(),
        status_code: code,
        output,
    })
}

pub async fn output_handler(
    req: actix_web::HttpRequest,
    query: web::Query<OutputQuery>,
    limiter: web::Data<RateLimiter>,
) -> HttpResponse {
    if let Err(resp) = verify_rate_limit(&req, &limiter) {
        return resp;
    }

    let workspace_id = &query.workspace_id;

    if !is_valid_uuid(workspace_id) {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    let workspace_dir = PathBuf::from("./workspace").join(workspace_id);

    if !workspace_dir.exists() {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    if let Err(e) = docker::ensure_running() {
        return HttpResponse::ServiceUnavailable().json(ErrorResponse {
            success: false,
            error_code: "DOCKER_UNAVAILABLE".to_string(),
            message: format!("Docker unavailable: {}", e),
        });
    }

    // Touch the workspace directory to reset the expiration timer
    let _ = fs::write(workspace_dir.join(".active"), b"");

    match docker::exec_zip_output(workspace_id) {
        Ok(bytes) => {
            HttpResponse::Ok()
                .content_type("application/octet-stream")
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}-output.zip\"", query.workspace_id),
                ))
                .body(bytes)
        }
        Err(e) => {
            if e == "WORKSPACE_NOT_FOUND" {
                HttpResponse::NotFound().json(ErrorResponse {
                    success: false,
                    error_code: "WORKSPACE_NOT_FOUND".to_string(),
                    message: "Output directory not found (compile may have failed)".to_string(),
                })
            } else {
                HttpResponse::InternalServerError().json(ErrorResponse {
                    success: false,
                    error_code: "DOCKER_EXEC_FAILED".to_string(),
                    message: format!("Failed to package build output: {}", e),
                })
            }
        }
    }
}

pub async fn delete_workspace_handler(
    req: actix_web::HttpRequest,
    path: web::Path<String>,
    limiter: web::Data<RateLimiter>,
) -> HttpResponse {
    if let Err(resp) = verify_rate_limit(&req, &limiter) {
        return resp;
    }

    let workspace_id = path.into_inner();

    if !is_valid_uuid(&workspace_id) {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    let workspace_dir = PathBuf::from("./workspace").join(&workspace_id);

    if !workspace_dir.exists() {
        return HttpResponse::NotFound().json(ErrorResponse {
            success: false,
            error_code: "WORKSPACE_NOT_FOUND".to_string(),
            message: "Workspace not found or expired".to_string(),
        });
    }

    let local_res = fs::remove_dir_all(&workspace_dir);
    let docker_res = docker::exec_cleanup(&workspace_id);

    if local_res.is_err() || docker_res.is_err() {
        return HttpResponse::InternalServerError().json(ErrorResponse {
            success: false,
            error_code: "FILE_WRITE_FAILED".to_string(),
            message: format!(
                "Failed to clean up workspace resources. Local error: {:?}, Docker error: {:?}",
                local_res.err(),
                docker_res.err()
            ),
        });
    }

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "workspace_id": workspace_id,
        "message": "Workspace deleted"
    }))
}
