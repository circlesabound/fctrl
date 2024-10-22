use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use fctrl::schema::ServerSettingsConfig;
use lazy_static::lazy_static;
use log::{error, info, warn};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{consts::*, error::Result, factorio::Factorio};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LaunchSettings {
    pub server_bind: SocketAddr,
    pub rcon_bind: SocketAddr,
    pub rcon_password: String,
    pub use_whitelist: bool,
}

impl LaunchSettings {
    pub async fn read() -> Result<Option<LaunchSettings>> {
        let path = &*LAUNCH_SETTINGS_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match toml::from_str::<LaunchSettings>(&s) {
                    Ok(launch_settings) => {
                        // ignore saved values for the binds, use defaults read from env vars
                        Ok(Some(LaunchSettings {
                            rcon_password: launch_settings.rcon_password,
                            ..Default::default()
                        }))
                    }
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

    pub async fn read_or_apply_default() -> Result<LaunchSettings> {
        match LaunchSettings::read().await? {
            Some(ls) => Ok(ls),
            None => {
                info!("Generating launch settings using defaults");
                let ls: LaunchSettings = Default::default();
                if let Err(e) = ls.write().await {
                    // this is okay
                    warn!("Failed to write default launch settings to file: {:?}", e);
                }
                Ok(ls)
            }
        }
    }

    pub async fn write(&self) -> Result<()> {
        let path = &*LAUNCH_SETTINGS_PATH;
        if let Err(e) = fs::create_dir_all(path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid launch settings path",
            )
        })?)
        .await
        {
            error!(
                "Error creating directory structure for launch settings: {:?}",
                e
            );
            return Err(e.into());
        }

        if let Err(e) = fs::write(path, toml::to_string(self)?).await {
            error!("Error writing launch settings: {:?}", e);
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

impl Default for LaunchSettings {
    fn default() -> Self {
        // Safe to unwrap these as they are checked by docker-compose
        let server_port = std::env::var(ENV_FACTORIO_PORT).unwrap().parse().unwrap();
        let rcon_port = std::env::var(ENV_FACTORIO_RCON_PORT)
            .unwrap()
            .parse()
            .unwrap();
        // generate random RCON password, 12 length, alphanumeric
        let rcon_password = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();
        LaunchSettings {
            server_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), server_port),
            rcon_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), rcon_port),
            rcon_password,
            use_whitelist: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Secrets {
    pub username: String,
    pub token: String,
}

impl Secrets {
    pub async fn read() -> Result<Option<Secrets>> {
        let path = &*SECRETS_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match toml::from_str(&s) {
                    Ok(secrets) => Ok(Some(secrets)),
                    Err(e) => {
                        error!("Error parsing secrets file: {:?}", e);
                        Err(e.into())
                    }
                },
                Err(e) => {
                    error!("Error reading secrets file: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn write(&self) -> Result<()> {
        let path = &*SECRETS_PATH;
        if let Err(e) = fs::create_dir_all(path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid secrets path")
        })?)
        .await
        {
            error!(
                "Error creating directory structure for secrets file: {:?}",
                e
            );
            return Err(e.into());
        }

        if let Err(e) = fs::write(path, toml::to_string(self)?).await {
            error!("Error writing secrets file: {:?}", e);
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

pub struct AdminList {
    pub list: Vec<String>,
    pub path: PathBuf,
}

impl AdminList {
    pub async fn read() -> Result<Option<AdminList>> {
        let path = &*ADMIN_LIST_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match serde_json::from_str(&s) {
                    Ok(list) => Ok(Some(AdminList {
                        list,
                        path: path.clone(),
                    })),
                    Err(e) => {
                        error!("Error parsing admin list: {:?}", e);
                        Err(e.into())
                    }
                },
                Err(e) => {
                    error!("Error reading admin list: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default() -> Result<AdminList> {
        match AdminList::read().await? {
            Some(adminlist) => Ok(adminlist),
            None => {
                info!("Generating admin list using defaults");
                let adminlist = AdminList {
                    list: vec![],
                    path: ADMIN_LIST_PATH.clone(),
                };
                if let Err(e) = adminlist.write().await {
                    // this is okay
                    warn!("Failed to write default admin list to file: {:?}", e);
                }
                Ok(adminlist)
            }
        }
    }

    pub async fn set(list: Vec<String>) -> Result<()> {
        let al = AdminList {
            list,
            path: ADMIN_LIST_PATH.clone(),
        };
        al.write().await
    }

    pub async fn write(&self) -> Result<()> {
        if let Err(e) = fs::create_dir_all(self.path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid admin list path")
        })?)
        .await
        {
            error!("Error creating directory structure for admin list: {:?}", e);
            return Err(e.into());
        }

        if let Err(e) = fs::write(&self.path, serde_json::to_string_pretty(&self.list)?).await {
            error!(
                "Error writing admin list to {}: {:?}",
                self.path.display(),
                e
            );
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

pub struct BanList {
    pub list: Vec<String>,
    pub path: PathBuf,
}

impl BanList {
    pub async fn read() -> Result<Option<BanList>> {
        let path = &*BAN_LIST_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match serde_json::from_str(&s) {
                    Ok(list) => Ok(Some(BanList {
                        list,
                        path: path.clone(),
                    })),
                    Err(e) => {
                        error!("Error parsing ban list: {:?}", e);
                        Err(e.into())
                    }
                },
                Err(e) => {
                    error!("Error reading ban list: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default() -> Result<BanList> {
        match BanList::read().await? {
            Some(banlist) => Ok(banlist),
            None => {
                info!("Generating ban list using defaults");
                let banlist = BanList {
                    list: vec![],
                    path: BAN_LIST_PATH.clone(),
                };
                if let Err(e) = banlist.write().await {
                    // this is okay
                    warn!("Failed to write default ban list to file: {:?}", e);
                }
                Ok(banlist)
            }
        }
    }

    pub async fn set(list: Vec<String>) -> Result<()> {
        let bl = BanList {
            list,
            path: BAN_LIST_PATH.clone(),
        };
        bl.write().await
    }

    pub async fn write(&self) -> Result<()> {
        if let Err(e) = fs::create_dir_all(self.path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid ban list path")
        })?)
        .await
        {
            error!("Error creating directory structure for ban list: {:?}", e);
            return Err(e.into());
        }

        if let Err(e) = fs::write(&self.path, serde_json::to_string_pretty(&self.list)?).await {
            error!("Error writing ban list to {}: {:?}", self.path.display(), e);
            Err(e.into())
        } else {
            Ok(())
        }
    }
}

pub struct WhiteList {
    pub list: Vec<String>,
    pub path: PathBuf,
}

impl WhiteList {
    pub async fn read() -> Result<Option<WhiteList>> {
        let path = &*WHITE_LIST_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => match serde_json::from_str(&s) {
                    Ok(list) => Ok(Some(WhiteList {
                        list,
                        path: path.clone(),
                    })),
                    Err(e) => {
                        error!("Error parsing white list: {:?}", e);
                        Err(e.into())
                    }
                },
                Err(e) => {
                    error!("Error reading white list: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default() -> Result<WhiteList> {
        match WhiteList::read().await? {
            Some(whitelist) => Ok(whitelist),
            None => {
                info!("Generating white list using defaults");
                let whitelist = WhiteList {
                    list: vec![],
                    path: WHITE_LIST_PATH.clone(),
                };
                if let Err(e) = whitelist.write().await {
                    // this is okay
                    warn!("Failed to write default white list to file: {:?}", e);
                }
                Ok(whitelist)
            }
        }
    }

    pub async fn set(list: Vec<String>) -> Result<()> {
        let wl = WhiteList {
            list,
            path: WHITE_LIST_PATH.clone(),
        };
        wl.write().await
    }

    pub async fn write(&self) -> Result<()> {
        if let Err(e) = fs::create_dir_all(self.path.parent().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid white list path")
        })?)
        .await
        {
            error!("Error creating directory structure for white list: {:?}", e);
            return Err(e.into());
        }

        if let Err(e) = fs::write(&self.path, serde_json::to_string_pretty(&self.list)?).await {
            error!(
                "Error writing white list to {}: {:?}",
                self.path.display(),
                e
            );
            Err(e.into())
        } else {
            Ok(())
        }
    }
}
pub struct ServerSettings {
    pub config: ServerSettingsConfig,
    pub path: PathBuf,
}

impl ServerSettings {
    pub async fn read() -> Result<Option<ServerSettings>> {
        let path = &*SERVER_SETTINGS_PATH;
        if !path.is_file() {
            Ok(None)
        } else {
            match fs::read_to_string(path).await {
                Ok(s) => {
                    match serde_json::from_str(&s) {
                        Ok(config) => Ok(Some(ServerSettings {
                            config,
                            path: path.clone(),
                        })),
                        Err(e) => {
                            error!("Error deserialising server settings: {:?}", e);
                            Err(e.into())
                        }
                    }
                },
                Err(e) => {
                    error!("Error reading server settings: {:?}", e);
                    Err(e.into())
                }
            }
        }
    }

    pub async fn read_or_apply_default(installation: &Factorio) -> Result<ServerSettings> {
        match ServerSettings::read().await? {
            Some(ls) => Ok(ls),
            None => {
                info!("Generating server settings using defaults");
                let mut config = ServerSettings::read_default_server_settings(installation).await?;
                // clear the default empty secrets
                config.username = None;
                config.token = None;
                let s = ServerSettings {
                    config,
                    path: SERVER_SETTINGS_PATH.clone(),
                };
                if let Err(e) = s.write().await {
                    error!("Failed to write default server settings to file: {:?}", e);
                    Err(e)
                } else {
                    Ok(s)
                }
            }
        }
    }

    pub async fn set(config: ServerSettingsConfig) -> Result<()> {
        let ss = ServerSettings {
            config,
            path: SERVER_SETTINGS_PATH.clone(),
        };
        ss.write().await
    }

    pub async fn write(&self) -> Result<()> {
        if let Err(e) = fs::create_dir_all(self.path.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid server settings path",
            )
        })?)
        .await
        {
            error!(
                "Error creating directory structure for server settings: {:?}",
                e
            );
            return Err(e.into());
        }

        let pretty_out = serde_json::to_string_pretty(&self.config)?;

        if let Err(e) = fs::write(&self.path, pretty_out).await {
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

    async fn read_default_server_settings(installation: &Factorio) -> Result<ServerSettingsConfig> {
        let path = installation
            .path
            .join("factorio")
            .join("data")
            .join("server-settings.example.json");
        match fs::read_to_string(path).await {
            Ok(s) => Ok(serde_json::from_str(&s)?),
            Err(e) => {
                error!("Error reading default server settings: {:?}", e);
                Err(e.into())
            }
        }
    }
}

lazy_static! {
    static ref LAUNCH_SETTINGS_PATH: PathBuf = CONFIG_DIR.join("launch-settings.toml");
    static ref ADMIN_LIST_PATH: PathBuf = CONFIG_DIR.join("server-adminlist.json");
    static ref BAN_LIST_PATH: PathBuf = CONFIG_DIR.join("server-banlist.json");
    static ref SERVER_SETTINGS_PATH: PathBuf = CONFIG_DIR.join("server-settings.json");
    static ref SECRETS_PATH: PathBuf = CONFIG_DIR.join("secrets.toml");
    static ref WHITE_LIST_PATH: PathBuf = CONFIG_DIR.join("server-whitelist.json");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_deserialise_and_serialise_launch_settings(
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();

        let ls = LaunchSettings {
            server_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 12345),
            rcon_bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 54321),
            rcon_password: "password123".to_owned(),
            use_whitelist: false,
        };
        let string_from_ls = toml::to_string(&ls)?;

        let string = r#"
server_bind = "0.0.0.0:12345"
rcon_bind = "127.0.0.1:54321"
rcon_password = "password123"
use_whitelist = false
"#
        .to_owned();
        let ls_from_string = toml::from_str(&string)?;

        assert_eq!(ls, ls_from_string);
        assert_eq!(string.trim(), string_from_ls.trim());

        Ok(())
    }
}
