use crate::{GEN_TAIL_MAX_ATTAMPS, error::AppError};
use deadpool_sqlite::{
    Pool,
    rusqlite::{Error, OptionalExtension},
};
use rand::distr::{Alphabetic, SampleString};

pub async fn init_db(db_pool: &Pool) -> Result<(), AppError> {
    let db_conn = db_pool.get().await?;
    db_conn
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
            return Ok::<(), Error>(());
        })
        .await??;
    return Ok(());
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
    let res = db_conn
        .interact(move |conn| {
            let tx = conn.transaction()?;

            let mut tail = None;
            for _ in 0..GEN_TAIL_MAX_ATTAMPS {
                let try_tail = Alphabetic.sample_string(&mut rand::rng(), tail_len);
                let exist = tx
                    .query_row("SELECT 1 FROM urls WHERE tail = ?1", (&try_tail,), |_| {
                        Ok(())
                    })
                    .optional()
                    .map(|o| o.is_some())?;

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

            return Ok::<_, AppError>(tail);
        })
        .await??;

    return Ok(res);
}

pub async fn cleanup_expired_url(db_pool: &Pool, now: i64) -> Result<Vec<String>, AppError> {
    let db_conn = db_pool.get().await?;
    let db_param = (now,);
    let res = db_conn
        .interact(move |conn| {
            let tx = conn.transaction()?;

            tx.execute(
                "CREATE TEMP TABLE expired_count AS
                SELECT file_sha256sum, COUNT(*) AS cnt
                FROM urls
                WHERE expires_at <= ?1
                GROUP BY file_sha256sum",
                db_param,
            )?;

            tx.execute("DELETE FROM urls WHERE expires_at <= ?1", db_param)?;

            tx.execute(
                "UPDATE files
                SET ref_count = ref_count - (
                    SELECT cnt
                    FROM expired_count
                    WHERE expired_count.file_sha256sum = files.file_sha256sum
                )
                WHERE file_sha256sum IN (SELECT file_sha256sum FROM expired_count)",
                (),
            )?;

            let mut stmt =
                tx.prepare("DELETE FROM files WHERE ref_count = 0 RETURNING file_sha256sum")?;
            let query_res = stmt.query_map((), |row| row.get::<_, String>(0))?;
            let mut res = Vec::new();
            for value in query_res {
                res.push(value?);
            }
            stmt.finalize()?;

            tx.execute("DROP TABLE expired_count", ())?;

            tx.commit()?;

            return Ok::<_, AppError>(res);
        })
        .await??;
    return Ok(res);
}

pub async fn get_file_by_url(
    db_pool: &Pool,
    tail: &str,
) -> Result<Option<(String, String)>, AppError> {
    let db_conn = db_pool.get().await?;
    let db_param = (tail.to_string(),);
    let res = db_conn
        .interact(move |conn| {
            return conn
                .query_row(
                    "SELECT file_sha256sum, mimetype FROM urls WHERE tail = ?1",
                    db_param,
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional();
        })
        .await??;
    return Ok(res);
}
