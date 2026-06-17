mod api;
mod docker;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer, middleware};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    match docker::ensure_running() {
        Ok(()) => println!("Container 'build-server' is running"),
        Err(e) => {
            eprintln!("Cannot start build container: {}", e);
            eprintln!("Make sure Docker Desktop is running and 'build-container' image exists");
            std::process::exit(1);
        }
    }

    println!("Listening on http://127.0.0.1:3001");

    HttpServer::new(|| {
        App::new()
            .wrap(Cors::permissive())
            .wrap(middleware::Logger::default())
            .app_data(web::PayloadConfig::new(100 * 1024 * 1024))
            .service(
                web::scope("/api")
                    .route("/sync", web::post().to(api::sync_handler))
                    .route("/compile", web::put().to(api::compile_handler))
                    .route("/output", web::get().to(api::output_handler))
            )
    })
    .bind("127.0.0.1:3001")?
    .run()
    .await
}
