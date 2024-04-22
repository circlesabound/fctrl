use std::path::{Path, PathBuf};

use fctrl::schema::{Save, SaveBytes};
use log::{error, info, warn};
use tokio::fs;

use crate::{consts::*, error::Result};

pub fn get_savefile_path(save_name: impl AsRef<str>) -> PathBuf {
    SAVEFILE_DIR.join(format!("{}.zip", save_name.as_ref()))
}

pub async fn delete_savefile(save_name: impl AsRef<str>) -> Result<()> {
    let path = get_savefile_path(save_name.as_ref());
    match fs::remove_file(path).await {
        Ok(()) => {
            info!("Successfully deleted savefile `{}`", save_name.as_ref());
            Ok(())
        },
        Err(e) => {
            error!("Failed to delete savefile `{}`: {:?}", save_name.as_ref(), e);
            Err(e.into())
        },
    }
}

pub async fn exists_savefile(save_name: impl AsRef<str>) -> Result<bool> {
    Ok(list_savefiles().await?.into_iter().find(|s| s.name == save_name.as_ref()).is_some())
}

pub async fn get_savefile(save_name: impl AsRef<str>) -> Result<Option<SaveBytes>> {
    if !SAVEFILE_DIR.is_dir() {
        return Ok(None);
    }

    let savefiles = list_savefiles().await?;
    match savefiles.into_iter().find(|s| s.name == save_name.as_ref()) {
        Some(s) => {
            let bytes = fs::read(get_savefile_path(s.name)).await?;
            Ok(Some(SaveBytes::new(bytes)))
        },
        None => Ok(None),
    }
}

pub async fn list_savefiles() -> Result<Vec<Save>> {
    if !SAVEFILE_DIR.is_dir() {
        return Ok(vec![]);
    }

    let mut ret = vec![];
    let mut entries = fs::read_dir(&*SAVEFILE_DIR).await?;
    while let Ok(Some(e)) = entries.next_entry().await {
        if let Ok(save) = parse_from_path(e.path()) {
            ret.push(save);
        } else {
            warn!("Invalid file {} found in save dir", e.path().display());
        }
    }

    Ok(ret)
}

pub async fn set_savefile(save_name: impl AsRef<str>, savebytes: SaveBytes) -> Result<()> {
    let bytes_length = savebytes.bytes.len();
    match fs::write(get_savefile_path(save_name.as_ref()), savebytes.bytes).await {
        Ok(()) => {
            info!("Successfully set savefile `{}`, wrote {} bytes", save_name.as_ref(), bytes_length);
            Ok(())
        },
        Err(e) => {
            error!("Failed to set savefile `{}`: {:?}", save_name.as_ref(), e);
            Err(e.into())
        }
    }
}

fn parse_from_path<P: AsRef<Path>>(path: P) -> Result<Save> {
    if let Some(ext) = path.as_ref().extension() {
        if ext == "zip" {
            let name = path
                .as_ref()
                .file_stem()
                .ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid path for save")
                })?
                .to_string_lossy()
                .into_owned();
            let last_modified = path.as_ref().metadata()?.modified()?.into();
            return Ok(Save {
                name,
                last_modified,
            });
        }
    }

    Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid save file").into())
}
