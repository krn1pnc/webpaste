use std::path::PathBuf;

use crate::conf;

pub fn get_full_path(filename: &str) -> PathBuf {
    return conf().upload_file_dir.join(filename);
}
