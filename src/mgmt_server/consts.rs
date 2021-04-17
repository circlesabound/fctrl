use std::path::PathBuf;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref DB_DIR: PathBuf = PathBuf::from("db");
}
