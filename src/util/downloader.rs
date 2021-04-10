use bytes::Bytes;
use std::path::PathBuf;
use tokio::fs;

pub async fn download<T: reqwest::IntoUrl>(id: &str, uri: T) -> crate::error::Result<Bytes> {
    if let Some(cached_bytes) = read_from_cache(id).await? {
        info!("Cache hit on {}", id);
        return Ok(cached_bytes);
    }

    match reqwest::get(uri).await {
        Ok(response) => match response.error_for_status() {
            Ok(response) => {
                let bytes = response.bytes().await.unwrap();
                info!("Download succesful, downloaded {} bytes", bytes.len());
                let save_path = get_cache_path().await?.join(id);
                fs::write(&save_path, &bytes).await?;
                info!("Cached at {}", save_path.display());
                Ok(bytes)
            }
            Err(e) => Err(e.into()),
        },
        Err(e) => Err(e.into()),
    }
}

pub async fn purge(id: &str) -> crate::error::Result<()> {
    let path = get_cache_path().await?.join(id);
    fs::remove_dir_all(path).await?;
    Ok(())
}

pub async fn purge_all() -> crate::error::Result<()> {
    let mut entries = fs::read_dir(get_cache_path().await?).await?;
    while let Some(entry) = entries.next_entry().await? {
        fs::remove_dir_all(entry.path()).await?;
    }
    Ok(())
}

async fn get_cache_path() -> crate::error::Result<PathBuf> {
    let cache_path = std::env::temp_dir().join("fctrl_downloader_cache");
    fs::create_dir_all(&cache_path).await?;
    Ok(cache_path)
}

async fn read_from_cache(id: &str) -> crate::error::Result<Option<Bytes>> {
    let cached_item_path = get_cache_path().await?.join(id);
    match fs::read(cached_item_path).await {
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
