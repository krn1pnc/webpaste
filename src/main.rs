use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::{Router, response::Html};
use deadpool_sqlite::{Config, Runtime};

use webpaste::{conf, init_config};
use webpaste::{handle_access, handle_upload, init_cleanup, init_db};

async fn handle_root() -> Html<&'static str> {
    return Html(include_str!("../index.html"));
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    nyquest_preset::register();

    init_config(std::env::args().nth(1).map(|s| PathBuf::from(s))).unwrap();

    let db_cfg = Config::new(&conf().database_file);
    let db_pool = Arc::new(db_cfg.create_pool(Runtime::Tokio1).unwrap());
    init_db(&db_pool).await.unwrap();

    init_cleanup(&db_pool);

    let app = Router::new()
        .route("/", get(handle_root))
        .route("/", post(handle_upload))
        .route("/{path}", get(handle_access))
        .with_state(db_pool);

    let listen_addr = &conf().listen_addr;
    let listener = tokio::net::TcpListener::bind(listen_addr).await.unwrap();

    tracing::info!("listening on {}", listen_addr);
    axum::serve(listener, app).await.unwrap();
}
