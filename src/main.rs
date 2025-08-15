use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    response::Html,
    routing::{get, post},
};
use chrono::Utc;
use deadpool_sqlite::{Config, Pool, Runtime};

use webpaste::{
    AppError, CLEANUP_DURATION, DATABASE_FILE, LISTEN_ADDR, UPLOAD_FILE_DIR, cleanup_expired_url,
};
use webpaste::{handle_access, handle_upload, init_db};

async fn handle_root() -> Html<&'static str> {
    return Html(include_str!("../index.html"));
}

async fn cleanup_urls(db_pool: &Pool) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    let cleanup_files = cleanup_expired_url(&db_pool, now).await?;
    for filename in cleanup_files {
        tokio::fs::remove_file(format!("{}/{}", UPLOAD_FILE_DIR, filename)).await?;
    }
    return Ok(());
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let db_cfg = Config::new(DATABASE_FILE);
    let db_pool = Arc::new(db_cfg.create_pool(Runtime::Tokio1).unwrap());
    init_db(&db_pool).await.unwrap();

    tokio::fs::create_dir(UPLOAD_FILE_DIR)
        .await
        .unwrap_or_else(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => (),
            other => panic!("error creating the uploads directory: {}", other),
        });

    let db_pool_cleanup = db_pool.clone();
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_DURATION as u64));
        loop {
            interval.tick().await;
            cleanup_urls(&db_pool_cleanup).await.unwrap();
        }
    });

    nyquest_preset::register();

    let app = Router::new()
        .route("/", get(handle_root))
        .route("/", post(handle_upload))
        .route("/{path}", get(handle_access))
        .with_state(db_pool);

    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR).await.unwrap();

    tracing::info!("listening on {}", LISTEN_ADDR);
    axum::serve(listener, app).await.unwrap();
}
