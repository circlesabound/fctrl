use std::path::PathBuf;

use lazy_static::lazy_static;

pub const ENV_AGENT_WS_PORT: &str = "AGENT_WS_PORT";
pub const ENV_FACTORIO_PORT: &str = "FACTORIO_PORT";
pub const ENV_FACTORIO_RCON_PORT: &str = "FACTORIO_RCON_PORT";

lazy_static! {
    pub static ref FACTORIO_INSTALL_DIR: PathBuf = PathBuf::from("install");
    pub static ref ROAMING_DATA_DIR: PathBuf = PathBuf::from("data");
    pub static ref CONFIG_DIR: PathBuf = ROAMING_DATA_DIR.join("configs");
    pub static ref MOD_DIR: PathBuf = ROAMING_DATA_DIR.join("mods");
    pub static ref SAVEFILE_DIR: PathBuf = ROAMING_DATA_DIR.join("saves");
}
