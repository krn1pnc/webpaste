use crate::error::AppError;
use crate::{GEN_TAIL_MAX_ATTAMPS, UPLOAD_FILE_DIR};

use deadpool_sqlite::Pool;
use deadpool_sqlite::rusqlite::OptionalExtension;
use rand::distr::{Alphabetic, SampleString};

pub async fn init_db(db_pool: &Pool) -> Result<(), AppError> {
    let db_conn = db_pool.get().await?;
    return db_conn
        .interact(|conn| {
            conn.execute(
                "CREATE TABLE IF NOT EXISTS files(
                    file_sha256sum TEXT PRIMARY KEY,
                    ref_count INTEGER
                )",
                (),
            )?;
            conn.execute(
                "CREATE TABLE IF NOT EXISTS urls(
                    tail TEXT PRIMARY KEY,
                    file_sha256sum TEXT,
                    mimetype TEXT,
                    expires_at INTEGER
                )",
                (),
            )?;
            conn.execute(
                "CREATE INDEX IF NOT EXISTS index_expires_at ON urls(expires_at)",
                (),
            )?;
            return Ok(());
        })
        .await?;
}

pub async fn add_url(
    db_pool: &Pool,
    tail_len: usize,
    file_sha256sum_: &str,
    mimetype_: &str,
    expires_at: i64,
) -> Result<String, AppError> {
    let file_sha256sum = file_sha256sum_.to_string();
    let mimetype = mimetype_.to_string();
    let db_conn = db_pool.get().await?;
    return db_conn
        .interact(move |conn| {
            let tx = conn.transaction()?;

            let mut tail = None;
            for _ in 0..GEN_TAIL_MAX_ATTAMPS {
                let try_tail = Alphabetic.sample_string(&mut rand::rng(), tail_len);
                let exist = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM urls WHERE tail = ?1)",
                    (&try_tail,),
                    |row| row.get::<_, bool>(0),
                )?;

                if !exist {
                    tail = Some(try_tail);
                    break;
                }
            }

            let tail = tail.ok_or(AppError::TailDrained)?;

            tx.execute(
                "INSERT INTO files VALUES (?1, 1) 
                ON CONFLICT DO UPDATE SET ref_count = ref_count + 1",
                (&file_sha256sum,),
            )?;

            tx.execute(
                "INSERT INTO urls VALUES (?1, ?2, ?3, ?4)",
                (&tail, &file_sha256sum, &mimetype, expires_at),
            )?;

            tx.commit()?;

            return Ok(tail);
        })
        .await?;
}

pub async fn cleanup_expired_urls(db_pool: &Pool, now: i64) -> Result<(), AppError> {
    let db_conn = db_pool.get().await?;
    return db_conn
        .interact(move |conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "WITH expired_count AS (
                    SELECT file_sha256sum, COUNT(*) AS decr
                    FROM urls
                    WHERE expires_at <= ?1
                    GROUP BY file_sha256sum
                )
                UPDATE files
                SET ref_count = ref_count - expired_count.decr
                FROM expired_count
                WHERE files.file_sha256sum = expired_count.file_sha256sum",
                (now,),
            )?;
            tx.execute("DELETE FROM urls WHERE expires_at <= ?1", (now,))?;
            tx.execute("DELETE FROM files WHERE ref_count = 0", ())?;
            tx.commit()?;
            return Ok(());
        })
        .await?;
}

pub async fn cleanup_unreachable_files(db_pool: &Pool) -> Result<(), AppError> {
    let db_conn = db_pool.get().await?;
    return db_conn
        .interact(|conn| {
            let upload_dir = std::fs::read_dir(UPLOAD_FILE_DIR)?;
            for entry in upload_dir {
                let path = entry?.path();
                if !path.is_file() {
                    continue;
                }

                let exist = conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM files WHERE file_sha256sum = ?1)",
                    (path.file_name().unwrap().to_string_lossy(),),
                    |row| row.get::<_, bool>(0),
                )?;

                if !exist {
                    std::fs::remove_file(path)?;
                }
            }
            return Ok(());
        })
        .await?;
}

pub async fn get_file_by_url(
    db_pool: &Pool,
    tail: &str,
) -> Result<Option<(String, String)>, AppError> {
    let db_conn = db_pool.get().await?;
    let db_param = (tail.to_string(),);
    return db_conn
        .interact(move |conn| {
            return conn
                .query_row(
                    "SELECT file_sha256sum, mimetype FROM urls WHERE tail = ?1",
                    db_param,
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|e| AppError::Sqlite(e));
        })
        .await?;
}
