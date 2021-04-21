use bytes::Bytes;
use log::{debug, error};
use std::time::Duration;
use std::{path::PathBuf, time::SystemTime};
use tokio::fs;

use crate::error::Result;

pub async fn download<T: reqwest::IntoUrl>(id: &str, uri: T) -> Result<Bytes> {
    if let Some(cached_bytes) = read_from_cache(id).await? {
        debug!("Cache hit on {}", id);
        return Ok(cached_bytes);
    }

    match reqwest::get(uri).await {
        Ok(response) => match response.error_for_status() {
            Ok(response) => {
                let bytes = response.bytes().await.unwrap();
                debug!("Download succesful, downloaded {} bytes", bytes.len());
                write_to_cache(id, &bytes).await?;
                Ok(bytes)
            }
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

pub async fn purge(id: &str) -> Result<()> {
    let path = get_cache_path().await?.join(id);
    if let Err(e) = fs::remove_file(path).await {
        if e.kind() == std::io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e.into())
        }
    } else {
        Ok(())
    }
}

pub async fn purge_all() -> Result<()> {
    let mut entries = fs::read_dir(get_cache_path().await?).await?;
    while let Some(entry) = entries.next_entry().await? {
        fs::remove_dir_all(entry.path()).await?;
    }
    Ok(())
}

async fn get_cache_path() -> Result<PathBuf> {
    let cache_path = std::env::temp_dir().join("fctrl_downloader_cache");
    fs::create_dir_all(&cache_path).await?;
    Ok(cache_path)
}

async fn read_from_cache(id: &str) -> Result<Option<Bytes>> {
    let cached_item_path = get_cache_path().await?.join(id);
    debug!(
        "Attempting to read metadata for {}",
        cached_item_path.display()
    );
    match fs::metadata(&cached_item_path).await {
        Ok(m) => {
            if m.created()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .elapsed()
                .unwrap_or_else(|_| Duration::new(u64::MAX, 0))
                > Duration::from_secs(60 * 60 * 24)
            {
                // if cached item older than a day, purge and refresh
                purge(id).await?;
                Ok(None)
            } else {
                match fs::read(&cached_item_path).await {
                    Ok(contents) => Ok(Some(contents.into())),
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            Ok(None)
                        } else {
                            Err(e.into())
                        }
                    }
                }
            }
        }
        Err(e) => {
            if e.kind() != std::io::ErrorKind::NotFound {
                error!("Error reading cached item metadata: {:?}", e);
            }
            // ignore
            Ok(None)
        }
    }
}

async fn write_to_cache(id: &str, bytes: &Bytes) -> Result<()> {
    let save_path = get_cache_path().await?.join(id);
    fs::write(&save_path, bytes).await?;
    debug!("Cached at {}", save_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_read_from_cache_after_write() -> std::result::Result<(), Box<dyn std::error::Error>>
    {
        fctrl::util::testing::logger_init();

        let id = "can_read_from_cache_after_write";
        let data = Bytes::from_static(b"test bytes");

        purge(id).await?;
        assert!(read_from_cache(id).await?.is_none());

        write_to_cache(id, &data).await?;
        assert!(read_from_cache(id).await?.is_some());

        Ok(())
    }
}
