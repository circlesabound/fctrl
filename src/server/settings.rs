use std::path::PathBuf;

use crate::factorio::Factorio;
use crate::consts::*;
use log::error;
use tokio::fs;

pub async fn read_server_settings() -> crate::error::Result<Option<String>> {
    let path = get_server_settings_path();
    if !path.exists() {
        Ok(None)
    } else {
        match fs::read_to_string(path).await {
            Ok(s) => Ok(Some(s)),
            Err(e) => {
                error!("Error reading server settings: {:?}", e);
                Err(e.into())
            }
        }
    }
}

pub async fn read_default_server_settings(installation: &Factorio) -> crate::error::Result<String> {
    let path = installation
        .path
        .join("factorio")
        .join("data")
        .join("server-settings.example.json");
    match fs::read_to_string(path).await {
        Ok(s) => Ok(s),
        Err(e) => {
            error!("Error reading default server settings: {:?}", e);
            Err(e.into())
        }
    }
}

pub async fn write_server_settings(server_settings_json: &str) -> crate::error::Result<()> {
    let path = get_server_settings_path();
    if let Err(e) = fs::create_dir_all(path.parent().unwrap()).await {
        error!("Error creating directory structure for server settings: {:?}", e);
        return Err(e.into());
    }

    if let Err(e) = fs::write(get_server_settings_path(), server_settings_json).await {
        error!("Error writing server settings: {:?}", e);
        Err(e.into())
    } else {
        Ok(())
    }
}

fn get_server_settings_path() -> PathBuf {
    CONFIG_DIR.join("server-settings.json")
}
