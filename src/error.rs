use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("database connection pool error: {0}")]
    Pool(#[from] deadpool_sqlite::PoolError),

    #[error("database interaction error: {0}")]
    Interaction(#[from] deadpool_sqlite::InteractError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] deadpool_sqlite::rusqlite::Error),

    #[error("io error: {0}")]
    IO(#[from] std::io::Error),

    #[error("multipart error: {0}")]
    Multipart(#[from] axum::extract::multipart::MultipartError),

    #[error("request error: {0}")]
    RequestError(#[from] nyquest::Error),

    #[error("magic error: {0}")]
    MagicError(String),

    #[error("no file uploaded")]
    NoFileUploaded,

    #[error("field has no name")]
    FieldHasNoName,

    #[error("{0}")]
    LenParseError(String),

    #[error("{0}")]
    ExpiresParseError(String),

    #[error("file too large")]
    FileTooLarge,

    #[error("tail drained")]
    TailDrained,

    #[error("tail not found")]
    TailNotFound,
}
