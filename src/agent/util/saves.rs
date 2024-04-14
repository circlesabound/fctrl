use std::path::{Path, PathBuf};

use fctrl::schema::{Save, SaveBytes};
use log::warn;
use tokio::fs;

use crate::{consts::*, error::Result};

pub fn get_savefile_path(save_name: impl AsRef<str>) -> PathBuf {
    SAVEFILE_DIR.join(format!("{}.zip", save_name.as_ref()))
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
