// default configs
const LISTEN_ADDR: &'static str = "127.0.0.1:3000";
const BASE_URL: &'static str = "http://127.0.0.1:3000";
const UPLOAD_FILE_DIR: &'static str = "./uploads";
const DATABASE_FILE: &'static str = "webpaste.db";
const GEN_TAIL_MAX_ATTAMPS: usize = 16;
const DEFAULT_TAIL_LEN: usize = 4;
const MIN_EXPIRE_AGE: i64 = 30 * 24 * 60 * 60;
const MAX_EXPIRE_AGE: i64 = 365 * 24 * 60 * 60;
const MAX_FILE_SIZE: usize = 512 * 1024 * 1024;
const CLEANUP_URLS_DURATION: u64 = 30;
const CLEANUP_FILES_DURATION: u64 = 60;

use std::path::{Path, PathBuf};

use crate::error::AppError;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize, Debug)]
struct ConfigFile {
    #[serde(default)]
    listen_addr: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    upload_file_dir: Option<String>,
    #[serde(default)]
    database_file: Option<String>,
    #[serde(default)]
    gen_tail_max_attamps: Option<i64>,
    #[serde(default)]
    default_tail_len: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_humantime_duration")]
    min_expire_duration: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_humantime_duration")]
    max_expire_duration: Option<i64>,
    #[serde(default)]
    max_file_size: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_humantime_duration")]
    cleanup_urls_duration: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_humantime_duration")]
    cleanup_files_duration: Option<i64>,
}

fn deserialize_humantime_duration<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(s) => humantime::parse_duration(&s)
            .map(|d| Some(d.as_secs() as i64))
            .map_err(|e| serde::de::Error::custom(e)),
        None => Ok(None),
    }
}

pub struct Config {
    pub listen_addr: String,
    pub base_url: String,
    pub upload_file_dir: PathBuf,
    pub database_file: PathBuf,
    pub gen_tail_max_attamps: usize,
    pub default_tail_len: usize,
    pub min_expire_duration: i64,
    pub max_expire_duration: i64,
    pub max_file_size: usize,
    pub cleanup_urls_duration: u64,
    pub cleanup_files_duration: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: LISTEN_ADDR.to_string(),
            base_url: BASE_URL.to_string(),
            upload_file_dir: PathBuf::from(UPLOAD_FILE_DIR),
            database_file: PathBuf::from(DATABASE_FILE),
            gen_tail_max_attamps: GEN_TAIL_MAX_ATTAMPS,
            default_tail_len: DEFAULT_TAIL_LEN,
            min_expire_duration: MIN_EXPIRE_AGE,
            max_expire_duration: MAX_EXPIRE_AGE,
            max_file_size: MAX_FILE_SIZE,
            cleanup_urls_duration: CLEANUP_URLS_DURATION,
            cleanup_files_duration: CLEANUP_FILES_DURATION,
        }
    }
}

fn read_config(path: &Path) -> Result<Config, AppError> {
    let config_str = std::fs::read_to_string(path)?;
    let c = toml::from_str::<ConfigFile>(&config_str)
        .map_err(|e| AppError::ConfigParseError(e.to_string()))?;
    return Ok(Config {
        listen_addr: c.listen_addr.unwrap_or(LISTEN_ADDR.to_string()),
        base_url: c.base_url.unwrap_or(BASE_URL.to_string()),
        upload_file_dir: PathBuf::from(c.upload_file_dir.unwrap_or(UPLOAD_FILE_DIR.to_string())),
        database_file: PathBuf::from(c.database_file.unwrap_or(DATABASE_FILE.to_string())),
        gen_tail_max_attamps: c
            .gen_tail_max_attamps
            .map(|v| v as usize)
            .unwrap_or(GEN_TAIL_MAX_ATTAMPS),
        default_tail_len: c
            .default_tail_len
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TAIL_LEN),
        min_expire_duration: c.min_expire_duration.unwrap_or(MIN_EXPIRE_AGE),
        max_expire_duration: c.max_expire_duration.unwrap_or(MAX_EXPIRE_AGE),
        max_file_size: c.max_file_size.map(|v| v as usize).unwrap_or(MAX_FILE_SIZE),
        cleanup_urls_duration: c
            .cleanup_urls_duration
            .map(|v| v as u64)
            .unwrap_or(CLEANUP_URLS_DURATION),
        cleanup_files_duration: c
            .cleanup_files_duration
            .map(|v| v as u64)
            .unwrap_or(CLEANUP_FILES_DURATION),
    });
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init_config(path: Option<PathBuf>) -> Result<(), AppError> {
    let config = match path {
        Some(path) => read_config(&path)?,
        None => Config::default(),
    };
    CONFIG.get_or_init(|| config);
    return Ok(());
}

pub fn conf() -> &'static Config {
    return CONFIG.get().unwrap();
}
