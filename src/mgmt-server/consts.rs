use std::path::PathBuf;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref DB_DIR: PathBuf = PathBuf::from("db");
    pub static ref DB_PATH: PathBuf = DB_DIR.join("main");
    pub static ref DB_SECONDARY_PATH: PathBuf = DB_DIR.join("main_secondary");
}
