mod api;
mod docker;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware};
use std::time::Duration;
use std::fs;
use std::path::Path;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let _ = fs::create_dir_all("./workspace");

    match docker::ensure_running() {
        Ok(()) => println!("Container 'build-container' is running"),
        Err(e) => {
            eprintln!("Cannot start build container: {}", e);
            eprintln!("Make sure Docker Desktop is running and 'build-container' image exists");
            std::process::exit(1);
        }
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            let workspace_dir = Path::new("./workspace");
            if let Ok(entries) = fs::read_dir(workspace_dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            let should_clean = if let Ok(created) = metadata.created() {
                                if let Ok(elapsed) = created.elapsed() {
                                    elapsed > Duration::from_secs(3600)
                                } else {
                                    false
                                }
                            } else if let Ok(modified) = metadata.modified() {
                                if let Ok(elapsed) = modified.elapsed() {
                                    elapsed > Duration::from_secs(3600)
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            if should_clean {
                                let path = entry.path();
                                if let Some(uuid_str) = path.file_name().and_then(|n| n.to_str()) {
                                    println!("Cleaning up expired workspace: {}", uuid_str);
                                    let _ = fs::remove_dir_all(&path);
                                    let _ = docker::exec_cleanup(uuid_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    println!("Listening on http://127.0.0.1:3001");

    let rate_limiter = web::Data::new(api::RateLimiter::new());

    HttpServer::new(move || {
        App::new()
            .app_data(rate_limiter.clone())
            .app_data(web::JsonConfig::default().limit(100 * 1024 * 1024))
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
            .wrap(Cors::permissive())
            .wrap(middleware::Logger::default())
            .service(
                web::scope("/api")
                    .route("/sync", web::post().to(api::sync_handler))
                    .route("/compile", web::put().to(api::compile_handler))
                    .route("/output", web::get().to(api::output_handler))
                    .route("/workspace/{workspace_id}", web::delete().to(api::delete_workspace_handler))
            )
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await
}
