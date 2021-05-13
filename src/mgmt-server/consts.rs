use std::path::PathBuf;

use lazy_static::lazy_static;

pub const DB_NAME: &str = "main";
pub const DB_SECONDARY_NAME: &str = "main_secondary";

lazy_static! {
    pub static ref DB_DIR: PathBuf = PathBuf::from("db");
}
