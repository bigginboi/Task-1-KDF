use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::docker;

#[derive(Deserialize)]
pub struct SyncQuery {
    source_type: String,
}

#[derive(Serialize)]
pub struct SyncResponse {
    success: bool,
    workspace_id: String,
    source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct CompileQuery {
    workspace_id: String,
    source_type: String,
}

#[derive(Serialize)]
pub struct CompileResponse {
    success: bool,
    workspace_id: String,
    status_code: i32,
    output: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct OutputQuery {
    workspace_id: String,
}

pub async fn sync_handler(
    body: web::Bytes,
    query: web::Query<SyncQuery>,
) -> HttpResponse {
    let source_type = &query.source_type;

    if !["c", "cpp", "rust"].contains(&source_type.as_str()) {
        return HttpResponse::BadRequest().json(SyncResponse {
            success: false,
            workspace_id: String::new(),
            source_type: source_type.clone(),
            error: Some(format!("invalid source type: {}", source_type)),
        });
    }

    if body.is_empty() {
        return HttpResponse::BadRequest().json(SyncResponse {
            success: false,
            workspace_id: String::new(),
            source_type: source_type.clone(),
            error: Some("no file data".to_string()),
        });
    }

    let uuid = Uuid::new_v4().to_string();
    let temp_zip = std::env::temp_dir().join(format!("{}.zip", uuid));

    if let Err(e) = std::fs::write(&temp_zip, &body) {
        return HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            workspace_id: uuid,
            source_type: source_type.clone(),
            error: Some(format!("save temp: {}", e)),
        });
    }

    if let Err(e) = docker::exec_mkdir(&uuid) {
        let _ = std::fs::remove_file(&temp_zip);
        return HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            workspace_id: uuid,
            source_type: source_type.clone(),
            error: Some(e),
        });
    }

    let temp_str = temp_zip.to_string_lossy().to_string();
    let container_zip = format!("/tmp/{}.zip", uuid);

    if let Err(e) = docker::copy_into(&temp_str, &container_zip) {
        let _ = std::fs::remove_file(&temp_zip);
        return HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            workspace_id: uuid,
            source_type: source_type.clone(),
            error: Some(e),
        });
    }

    let _ = std::fs::remove_file(&temp_zip);

    if let Err(e) = docker::exec_unzip(&uuid) {
        return HttpResponse::InternalServerError().json(SyncResponse {
            success: false,
            workspace_id: uuid,
            source_type: source_type.clone(),
            error: Some(e),
        });
    }

    HttpResponse::Ok().json(SyncResponse {
        success: true,
        workspace_id: uuid,
        source_type: source_type.clone(),
        error: None,
    })
}

pub async fn compile_handler(
    query: web::Query<CompileQuery>,
) -> HttpResponse {
    let (success, code, output) = docker::exec_compile(&query.workspace_id, &query.source_type);

    HttpResponse::Ok().json(CompileResponse {
        success,
        workspace_id: query.workspace_id.clone(),
        status_code: code,
        output,
        error: if success { None } else { Some("compilation failed".to_string()) },
    })
}

pub async fn output_handler(
    query: web::Query<OutputQuery>,
) -> HttpResponse {
    match docker::exec_zip_output(&query.workspace_id) {
        Ok(bytes) => {
            HttpResponse::Ok()
                .content_type("application/octet-stream")
                .append_header((
                    "Content-Disposition",
                    format!("attachment; filename=\"{}.zip\"", query.workspace_id),
                ))
                .body(bytes)
        }
        Err(e) => {
            HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "workspace_id": query.workspace_id,
                "error": e
            }))
        }
    }
}
