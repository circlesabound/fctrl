use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use crate::consts::*;
use crate::factorio::Factorio;
use lazy_static::lazy_static;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LaunchSettings {
    pub server_bind: SocketAddr,
    pub rcon_bind: SocketAddr,
    pub rcon_password: String,
}

impl LaunchSettings {
    pub async fn read() -> crate::error::Result<Option<LaunchSettings>> {
        let path = &*LAUNCH_SETTINGS_PATH;
        if !path.exists() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match toml::from_str(&s) {
                    Ok(launch_settings) => Ok(Some(launch_settings)),
                    Err(e) => {
                        error!("Error parsing launch settings: {:?}", e);
                        Err(e.into())
                    }
                },
                Err(e) => {
                    error!("Error reading launch settings: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default() -> crate::error::Result<LaunchSettings> {
        match LaunchSettings::read().await {
            Ok(Some(ls)) => Ok(ls),
            Ok(None) => {
                info!("Generating launch settings using defaults");
                let ls: LaunchSettings = Default::default();
                if let Err(e) = ls.write().await {
                    // this is okay
                    warn!("Failed to write default launch settings to file: {:?}", e);
                }
                Ok(ls)
            }
            Err(e) => {
                error!("Error: failed to read launch settings file: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn write(&self) -> crate::error::Result<()> {
        let path = &*LAUNCH_SETTINGS_PATH;
        if let Err(e) = fs::create_dir_all(path.parent().unwrap()).await {
            error!(
                "Error creating directory structure for launch settings: {:?}",
                e
            );
            return Err(e.into());
        }

        if let Err(e) = fs::write(path, toml::to_string(self).unwrap()).await {
            error!("Error writing launch settings: {:?}", e);
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

impl Default for LaunchSettings {
    fn default() -> Self {
        LaunchSettings {
            server_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 34197),
            rcon_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 7266),
            rcon_password: "rcon".to_owned(),
        }
    }
}

pub struct ServerSettings {
    pub json: String,
    pub path: PathBuf,
}

impl ServerSettings {
    pub async fn read() -> crate::error::Result<Option<ServerSettings>> {
        let path = &*SERVER_SETTINGS_PATH;
        if !path.exists() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => Ok(Some(ServerSettings {
                    json: s,
                    path: path.clone(),
                })),
                Err(e) => {
                    error!("Error reading server settings: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default(
        installation: &Factorio,
    ) -> crate::error::Result<ServerSettings> {
        match ServerSettings::read().await {
            Ok(Some(ls)) => Ok(ls),
            Ok(None) => {
                info!("Generating server settings using defaults");
                let defaults = ServerSettings::read_default_server_settings(installation).await?;
                let s = ServerSettings {
                    json: defaults,
                    path: SERVER_SETTINGS_PATH.clone(),
                };
                if let Err(e) = s.write().await {
                    error!("Failed to write default server settings to file: {:?}", e);
                    Err(e.into())
                } else {
                    Ok(s)
                }
            }
            Err(e) => {
                error!("Error: failed to read server settings file: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn write(&self) -> crate::error::Result<()> {
        if let Err(e) = fs::create_dir_all(self.path.parent().unwrap()).await {
            error!(
                "Error creating directory structure for server settings: {:?}",
                e
            );
            return Err(e.into());
        }

        if let Err(e) = fs::write(&self.path, &self.json).await {
            error!(
                "Error writing server settings to {}: {:?}",
                self.path.display(),
                e
            );
            Err(e.into())
        } else {
            Ok(())
        }
    }

    async fn read_default_server_settings(installation: &Factorio) -> crate::error::Result<String> {
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
}

lazy_static! {
    static ref LAUNCH_SETTINGS_PATH: PathBuf = CONFIG_DIR.join("launch-settings.toml");
    static ref SERVER_SETTINGS_PATH: PathBuf = CONFIG_DIR.join("server-settings.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_deserialise_and_serialise_launch_settings() -> Result<(), Box<dyn std::error::Error>> {
        crate::util::testing::logger_init();

        let ls = LaunchSettings {
            server_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 12345),
            rcon_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 54321),
            rcon_password: "password123".to_owned(),
        };
        let string_from_ls = toml::to_string(&ls)?;

        let string = r#"
server_bind = "0.0.0.0:12345"
rcon_bind = "127.0.0.1:54321"
rcon_password = "password123"
"#
        .to_owned();
        let ls_from_string = toml::from_str(&string)?;

        assert_eq!(ls, ls_from_string);
        assert_eq!(string.trim(), string_from_ls.trim());

        Ok(())
    }
}
