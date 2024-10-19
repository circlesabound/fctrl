use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

use bytes::Buf;
use log::{error, info, warn};
use tar::Archive;
use tokio::fs;
use xz2::read::XzDecoder;

use crate::{error::Result, util};

/// Represents an installation of Factorio headless server software
pub struct Factorio {
    pub path: PathBuf,
    pub version: String,
}

pub struct VersionManager {
    install_dir: PathBuf,
    pub versions: HashMap<String, Factorio>,
}

impl VersionManager {
    pub async fn new<P: AsRef<Path>>(install_dir: P) -> Result<VersionManager> {
        let mut versions = HashMap::new();

        // create install dir if not exists
        fs::create_dir_all(&install_dir).await?;

        // Scan install dir for versions on disk
        let mut entries = fs::read_dir(&install_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().is_dir() {
                if let Some(dir_name) = entry.file_name().to_str() {
                    if let Some(version) = dir_name.strip_prefix("factorio_headless_x64_") {
                        let factorio_installation = Factorio {
                            path: entry.path(),
                            version: version.to_string(),
                        };
                        info!(
                            "VersionManager scan found version {} with path {}",
                            factorio_installation.version,
                            factorio_installation.path.display()
                        );
                        versions.insert(version.to_string(), factorio_installation);
                    }
                } else {
                    warn!(
                        "Could not convert {:?} to &str, VersionManager scan skipping this dir",
                        entry.file_name()
                    );
                }
            }
        }

        Ok(VersionManager {
            install_dir: install_dir.as_ref().to_path_buf(),
            versions,
        })
    }

    pub async fn install(&mut self, version: String) -> Result<()> {
        let uri = format!(
            "https://factorio.com/get-download/{}/headless/linux64",
            version
        );
        info!("Attempting to download version {} from {}", version, uri);
        let xz_bytes =
            util::downloader::download(&format!("{}.tar.xz", &VersionManager::get_download_id(&version)), uri).await?;

        // decompress in memory
        let decompress = XzDecoder::new(xz_bytes.reader());

        // extract tar archive and write files to install location
        let install_path = self.get_install_path(&version);
        info!("Attempting to install to {}", install_path.display());
        let mut tar = Archive::new(decompress);
        if let Err(e) = tar.unpack(&install_path) {
            error!("Error unpacking tar: {:?}", e);
            Err(e.into())
        } else {
            let new_installation = Factorio {
                path: install_path,
                version: version.clone(),
            };
            self.versions.insert(version, new_installation);
            Ok(())
        }
    }

    pub async fn delete(&mut self, version: &str) -> Result<()> {
        if let Some(installation) = self.versions.get(version) {
            fs::remove_dir_all(&installation.path).await?;
            self.versions.remove(version);
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("version {} does not exist", version),
            )
            .into())
        }
    }

    fn get_install_path(&self, version: impl AsRef<str>) -> PathBuf {
        self.install_dir.join(VersionManager::get_download_id(version))
    }

    fn get_download_id(version: impl AsRef<str>) -> String {
        if VersionManager::is_new_file_scheme(version.as_ref()) {
            format!("factorio-headless_linux_{}", version.as_ref())
        } else {
            format!("factorio_headless_x64_{}", version.as_ref())
        }
    }

    /// New naming scheme for dedi server files from 1.2.x onwards.
    /// Not sure if intentional but handle it anyway
    fn is_new_file_scheme(version: impl AsRef<str>) -> bool {
        version.as_ref().starts_with("2.") || version.as_ref().starts_with("1.2")
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[tokio::test]
    async fn can_install_version_1_1_104() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();

        let tmp_dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        fs::create_dir(&tmp_dir).await?;
        let mut vm = VersionManager::new(&tmp_dir).await?;
        vm.install("1.1.104".to_owned()).await?;

        assert!(vm.versions.contains_key("1.1.104"));

        let _ = fs::remove_dir_all(tmp_dir).await;

        Ok(())
    }
}
