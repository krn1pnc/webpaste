use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Router, response::Html};
use deadpool_sqlite::{Config, Runtime};

use webpaste::{DATABASE_FILE, LISTEN_ADDR};
use webpaste::{handle_access, handle_upload, init_cleanup, init_db};

async fn handle_root() -> Html<&'static str> {
    return Html(include_str!("../index.html"));
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    nyquest_preset::register();

    let db_cfg = Config::new(DATABASE_FILE);
    let db_pool = Arc::new(db_cfg.create_pool(Runtime::Tokio1).unwrap());
    init_db(&db_pool).await.unwrap();

    init_cleanup(&db_pool);

    let app = Router::new()
        .route("/", get(handle_root))
        .route("/", post(handle_upload))
        .route("/{path}", get(handle_access))
        .with_state(db_pool);

    let listener = tokio::net::TcpListener::bind(LISTEN_ADDR).await.unwrap();

    tracing::info!("listening on {}", LISTEN_ADDR);
    axum::serve(listener, app).await.unwrap();
}
