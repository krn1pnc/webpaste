use std::sync::Arc;

use crate::UPLOAD_FILE_DIR;
use crate::db;
use crate::error::AppError;

use axum::{
    body::Body,
    extract::{Path, State},
    http,
    response::IntoResponse,
};
use deadpool_sqlite::Pool;

async fn get_file(db_pool: &Pool, tail: &str) -> Result<(Vec<u8>, String), AppError> {
    let (filename, mimetype) = db::get_file_by_url(&db_pool, &tail)
        .await?
        .ok_or(AppError::TailNotFound)?;
    let data = tokio::fs::read(format!("{}/{}", UPLOAD_FILE_DIR, filename)).await?;
    Ok((data, mimetype))
}

pub async fn handle_access(
    State(db_pool): State<Arc<Pool>>,
    Path(tail): Path<String>,
) -> http::Response<Body> {
    match get_file(&db_pool, &tail).await {
        Ok((data, mimetype)) => return ([("Content-Type", mimetype)], data).into_response(),
        Err(e) => match e {
            AppError::TailNotFound => {
                return http::StatusCode::NOT_FOUND.into_response();
            }
            _ => {
                tracing::error!("{}", e);
                return http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
    }
}
