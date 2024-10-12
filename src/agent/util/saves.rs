use std::{convert::TryFrom, io::SeekFrom, path::{Path, PathBuf}};

use async_zip::tokio::read::fs::ZipFileReader;
use factorio_file_parser::SaveHeader;
use fctrl::schema::{Save, SaveBytes};
use futures::AsyncReadExt;
use log::{error, info, warn};
use tokio::{fs::{self, OpenOptions}, io::{AsyncSeekExt, AsyncWriteExt}};

use crate::{consts::*, error::{Error, Result}};

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
    if let Some(start_byte) = savebytes.multipart_start {
        // partial file write
        let filename = get_savefile_path(save_name.as_ref());
        // create if not exist
        let mut file = OpenOptions::new().write(true).create(true).open(filename).await?;
        if savebytes.is_sentinel() {
            // finalise and trim down to size
            file.set_len(start_byte as u64).await?;
            info!("Successfully finalised savefile `{}`, final length {} bytes", save_name.as_ref(), start_byte);
        } else {
            // seek to correct write location before writing
            file.seek(SeekFrom::Start(start_byte as u64)).await?;
            file.write_all(&savebytes.bytes).await?;
            file.flush().await?;
            info!("Successfully wrote to savefile `{}`, wrote {} bytes from offset {}", save_name.as_ref(), savebytes.bytes.len(), start_byte);
        }
        Ok(())
    } else {
        // write the whole file
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
}

pub async fn read_header(save_name: impl AsRef<str>) -> Result<SaveHeader> {
    // 1. open zip
    let reader = ZipFileReader::new(get_savefile_path(save_name.as_ref())).await?;
    for index in 0..reader.file().entries().len() {
        let entry = reader.file().entries().get(index).unwrap();
        // 2. locate level-init.dat and read into memory
        if let Ok(filename_str) = entry.filename().as_str() {
            if filename_str.ends_with("level-init.dat") {
                let mut entry_reader = reader.reader_without_entry(index).await?;
                let mut buf = vec![];
                entry_reader.read_to_end(&mut buf).await?;
                // 3. Parse as SaveHeader
                let save_header = SaveHeader::try_from(buf.as_ref())?;
                return Ok(save_header);
            }
        } else {
            warn!("unable to convert zip entry filename '{:?}' to UTF-8, skipping", entry.filename());
        }
    }
    Err(Error::HeaderNotFound)
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
