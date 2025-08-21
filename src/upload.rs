use std::sync::Arc;

use crate::db::add_url;
use crate::error::AppError;
use crate::{
    BASE_URL, DEFAULT_TAIL_LEN, MAX_EXPIRE_AGE, MAX_FILE_SIZE, MIN_EXPIRE_AGE, UPLOAD_FILE_DIR,
};

use axum::{
    body::{Body, Bytes},
    extract::{Multipart, State},
    http,
    response::IntoResponse,
};
use chrono::Utc;
use deadpool_sqlite::Pool;
use humantime::parse_duration;
use sha2::{Digest, Sha256};

fn calc_retention(size: usize) -> i64 {
    return (MIN_EXPIRE_AGE as f64
        + (MIN_EXPIRE_AGE - MAX_EXPIRE_AGE) as f64
            * (size as f64 / MAX_FILE_SIZE as f64 - 1.).powf(3.)) as i64;
}

pub fn guess_mime(data: &[u8]) -> Result<String, AppError> {
    let cookie = magic::Cookie::open(magic::cookie::Flags::MIME_TYPE)
        .map_err(|e| AppError::MagicError(e.to_string()))?;
    let cookie = cookie
        .load(&Default::default())
        .map_err(|e| AppError::MagicError(e.to_string()))?;
    let mut mimetype = cookie
        .buffer(&data)
        .map_err(|e| AppError::MagicError(e.to_string()))?;
    if mimetype == "text/plain" {
        let mut encdet = chardetng::EncodingDetector::new();
        encdet.feed(&data, true);
        mimetype += &format!("; charset={}", encdet.guess(None, true).name());
    }
    return Ok(mimetype);
}

async fn parse_multipart(mut multipart: Multipart) -> Result<(Bytes, usize, i64), AppError> {
    let mut data = None;
    let mut tail_len = DEFAULT_TAIL_LEN;
    let mut expires = None;

    while let Some(field) = multipart.next_field().await? {
        let name = field.name().ok_or(AppError::FieldHasNoName)?;
        match name {
            "file" => match data {
                None => data = Some(field.bytes().await?),
                Some(_) => continue,
            },
            "url" => match data {
                None => {
                    data = Some(Bytes::from_owner(
                        nyquest::r#async::get(field.text().await?)
                            .await?
                            .bytes()
                            .await?,
                    ))
                }
                Some(_) => continue,
            },
            "tail_len" => {
                tail_len = field
                    .text()
                    .await?
                    .parse::<usize>()
                    .map_err(|e| AppError::ParseError(e.to_string()))?
            }
            "expires" => match expires {
                None => expires = Some(field.text().await?),
                Some(_) => continue,
            },
            _ => (),
        }
    }

    let data = data.ok_or(AppError::NoFileUploaded)?;

    let now = Utc::now().timestamp();
    let expires_at = match expires {
        Some(expires) => match expires.chars().all(|c| c.is_numeric()) {
            true => expires
                .parse::<i64>()
                .map_err(|e| AppError::ParseError(e.to_string()))?,
            false => {
                now + parse_duration(&expires)
                    .map_err(|e| AppError::ParseError(e.to_string()))?
                    .as_secs() as i64
            }
        },
        None => now + calc_retention(data.len()),
    };

    return Ok((data, tail_len, expires_at));
}

async fn upload(db_pool: &Pool, multipart: Multipart) -> Result<String, AppError> {
    let (data, tail_len, expires_at) = parse_multipart(multipart).await?;
    if data.len() > MAX_FILE_SIZE {
        return Err(AppError::FileTooLarge);
    }

    let file_sha256sum = hex::encode(Sha256::digest(&data));
    let mimetype = guess_mime(&data)?;

    let tail = add_url(&db_pool, tail_len, &file_sha256sum, &mimetype, expires_at).await?;

    match tokio::fs::write(format!("{}/{}", UPLOAD_FILE_DIR, file_sha256sum), &data).await {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::AlreadyExists => Ok(()),
            _ => Err(e),
        },
    }?;

    return Ok(tail);
}

pub async fn handle_upload(
    State(db_pool): State<Arc<Pool>>,
    multipart: Multipart,
) -> http::Response<Body> {
    match upload(&db_pool, multipart).await {
        Ok(tail) => return format!("{}/{}\n", BASE_URL, tail).into_response(),
        Err(e) => match e {
            AppError::Multipart(e) => return e.status().into_response(),
            AppError::RequestError(e) => match e {
                nyquest::Error::InvalidUrl => {
                    return (http::StatusCode::BAD_REQUEST, "url is invalid\n").into_response();
                }
                nyquest::Error::ResponseTooLarge => {
                    return (http::StatusCode::BAD_REQUEST, "response too large\n").into_response();
                }
                nyquest::Error::RequestTimeout => {
                    return (http::StatusCode::SERVICE_UNAVAILABLE, "request timeout\n")
                        .into_response();
                }
                nyquest::Error::NonSuccessfulStatusCode(c) => {
                    return (
                        http::StatusCode::SERVICE_UNAVAILABLE,
                        format!("request failed with status code {}\n", c.code()),
                    )
                        .into_response();
                }
                other_error => {
                    tracing::error!("{}", other_error);
                    return http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            },
            AppError::NoFileUploaded => {
                return (
                    http::StatusCode::BAD_REQUEST,
                    "no 'file' or 'url' specified\n",
                )
                    .into_response();
            }
            AppError::FieldHasNoName => return http::StatusCode::BAD_REQUEST.into_response(),
            AppError::ParseError(msg) => {
                return (http::StatusCode::BAD_REQUEST, msg).into_response();
            }
            AppError::FileTooLarge => return http::StatusCode::PAYLOAD_TOO_LARGE.into_response(),
            AppError::TailDrained => {
                return (
                    http::StatusCode::SERVICE_UNAVAILABLE,
                    "cannot generate an unique url, try specifying a larger 'tail_len'\n",
                )
                    .into_response();
            }
            other_error => {
                tracing::error!("{}", other_error);
                return http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        },
    }
}
