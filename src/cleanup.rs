use std::sync::Arc;
use std::time::Duration;

use crate::CLEANUP_URLS_DURATION;
use crate::db::{cleanup_expired_urls, cleanup_unreachable_files};
use crate::error::AppError;

use chrono::Utc;
use deadpool_sqlite::Pool;

async fn cleanup_urls(db_pool: &Pool) -> Result<(), AppError> {
    let now = Utc::now().timestamp();
    cleanup_expired_urls(&db_pool, now).await?;
    return Ok(());
}

async fn cleanup_files(db_pool: &Pool) -> Result<(), AppError> {
    cleanup_unreachable_files(&db_pool).await?;
    return Ok(());
}

fn init_cleanup_urls(db_pool: Arc<Pool>) {
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_URLS_DURATION as u64));
        loop {
            interval.tick().await;
            match cleanup_urls(&db_pool).await {
                Ok(_) => (),
                Err(e) => tracing::error!("{}", e),
            };
        }
    });
}

fn init_cleanup_files(db_pool: Arc<Pool>) {
    tokio::task::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_URLS_DURATION as u64));
        loop {
            interval.tick().await;
            match cleanup_files(&db_pool).await {
                Ok(_) => (),
                Err(e) => tracing::error!("{}", e),
            };
        }
    });
}

pub fn init_cleanup(db_pool: &Arc<Pool>) {
    init_cleanup_urls(db_pool.clone());
    init_cleanup_files(db_pool.clone());
}
