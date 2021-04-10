use std::path::{Path, PathBuf};

use crate::consts::*;

pub fn get_save_dir_path() -> PathBuf {
    Path::new(DATA_DIR).join(SAVEFILE_SUBDIR)
}

pub fn get_savefile_path(save_name: &str) -> PathBuf {
    get_save_dir_path().join(format!("{}.zip", save_name))
}
