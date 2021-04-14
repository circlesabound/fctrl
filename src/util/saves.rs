use std::path::{Path, PathBuf};

use crate::consts::*;

pub fn get_savefile_path(save_name: &str) -> PathBuf {
    SAVEFILE_DIR.join(format!("{}.zip", save_name))
}
