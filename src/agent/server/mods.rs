use std::{
    collections::HashSet,
    convert::{TryFrom, TryInto},
    path::{Path, PathBuf},
};

use factorio_mod_settings_parser::ModSettings;
use futures::future;
use lazy_static::lazy_static;
use log::{debug, error, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{
    consts::*,
    error::{Error, Result},
    util::downloader,
};

use fctrl::schema::*;

use super::settings::Secrets;

lazy_static! {
    static ref MOD_LIST_PATH: PathBuf = MOD_DIR.join("mod-list.json");
    static ref MOD_SETTINGS_PATH: PathBuf = MOD_DIR.join("mod-settings.dat");
}

pub struct ModManager {
    pub mods: Vec<Mod>,
    pub settings: Option<ModSettings>,
    pub path: PathBuf,
}

impl ModManager {
    pub async fn read() -> Result<Option<ModManager>> {
        if !MOD_DIR.is_dir() {
            Ok(None)
        } else {
            // Don't bother with mod list, directly parse the mod zips
            // No support for "installed but disabled" mods
            let mut mod_zip_names = vec![];
            let mut entries = fs::read_dir(&*MOD_DIR).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path != *MOD_LIST_PATH && path != *MOD_SETTINGS_PATH {
                    mod_zip_names.push(path.file_name().unwrap().to_str().unwrap().to_string());
                }
            }

            // Parse each mod zip file name into mod name and version
            let mods = mod_zip_names
                .into_iter()
                .filter_map(|n| Mod::try_from_filename(&n))
                .collect();

            // mod settings is optional
            let mut settings = None;
            if MOD_SETTINGS_PATH.is_file() {
                let bytes = fs::read(&*MOD_SETTINGS_PATH).await?;
                match ModSettings::try_from(bytes.as_ref()) {
                    Ok(s) => settings = Some(s),
                    Err(e) => {
                        error!("Error parsing mod settings: {:?}", e);
                        return Err(e.into());
                    }
                }
            }

            Ok(Some(ModManager {
                mods,
                settings,
                path: MOD_DIR.clone(),
            }))
        }
    }

    pub async fn read_or_apply_default() -> Result<ModManager> {
        match ModManager::read().await? {
            Some(m) => Ok(m),
            None => {
                info!("Generating mod dir and contents using defaults");

                let ret = ModManager {
                    mods: vec![],
                    settings: None,
                    path: MOD_DIR.clone(),
                };
                ret.apply_without_download().await?;
                Ok(ret)
            }
        }
    }

    pub async fn apply(&self, secrets: &Secrets) -> Result<()> {
        // Read current mods, figure out the delta
        let currently_installed = ModManager::read().await?.map_or(vec![], |m| m.mods);
        let ModDelta { install, delete } =
            ModManager::calculate_mod_delta(&currently_installed, &self.mods);

        info!(
            "Mods to install: {}",
            install
                .iter()
                .map(|m| format!("{}_{}", m.name, m.version))
                .collect::<Vec<_>>()
                .join(", ")
        );
        info!(
            "Mods to delete: {}",
            delete
                .iter()
                .map(|m| format!("{}_{}", m.name, m.version))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Start tasks to install
        let mut tasks = vec![];
        for install in install.into_iter() {
            let install_path = self.path.clone();
            let secrets_clone = secrets.clone();
            tasks.push(tokio::spawn(async move {
                ModManager::download_mod(&install, &install_path, &secrets_clone).await
            }));
        }

        for delete in delete.into_iter() {
            let full_path = self
                .path
                .join(format!("{}_{}.zip", delete.name, delete.version));
            tasks.push(tokio::spawn(async move {
                Ok(fs::remove_file(full_path).await?)
            }));
        }

        // Apply metadata changes regardless of actual success or failure
        self.apply_without_download().await?;

        let mut errors = vec![];
        let results = future::join_all(tasks).await;
        for result in results {
            match result {
                Ok(result) => {
                    if let Err(e) = result {
                        error!("Failed to apply mod change: {:?}", e);
                        errors.push(e);
                    }
                }
                Err(e) => {
                    // task was cancelled or panicked. Nothing much we can do
                    error!("Join error applying mod changes: {:?}", e);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Aggregate(errors))
        }
    }

    async fn apply_without_download(&self) -> Result<()> {
        fs::create_dir_all(&*MOD_DIR).await?;

        // ModList impl automatically adds the base mod to its internal structure
        let mod_list_json = serde_json::to_string(&ModList::from(self.mods.clone()))?;
        fs::write(&*MOD_LIST_PATH, mod_list_json).await?;

        if let Some(settings) = self.settings.clone() {
            let bytes: Vec<u8> = settings.try_into()?;
            fs::write(&*MOD_SETTINGS_PATH, bytes).await?;
        }

        Ok(())
    }

    async fn short_query_mod(mod_to_query: &Mod) -> Result<factorio_mod_portal_api::ModInfoShort> {
        let short_query_url = format!("https://mods.factorio.com/api/mods/{}", mod_to_query.name);

        debug!("Querying mod {} at {}", mod_to_query.name, short_query_url);
        let short_query_response = reqwest::get(short_query_url).await?.error_for_status()?;
        Ok(short_query_response
            .json::<factorio_mod_portal_api::ModInfoShort>()
            .await?)
    }

    async fn download_mod<P: AsRef<Path>>(
        mod_to_download: &Mod,
        destination_dir: P,
        secrets: &Secrets,
    ) -> Result<()> {
        let info = ModManager::short_query_mod(&mod_to_download).await?;
        if let Some(r) = info
            .releases
            .iter()
            .find(|r| r.version == mod_to_download.version)
        {
            // Construct actual download url
            let download_url = format!(
                "https://mods.factorio.com/{}?username={}&token={}",
                r.download_url, secrets.username, secrets.token,
            );
            let filename = format!("{}_{}.zip", mod_to_download.name, mod_to_download.version);
            let out_file = destination_dir.as_ref().join(&filename);
            let bytes = downloader::download(&filename, download_url).await?;
            fs::write(&out_file, bytes).await?;
            info!(
                "Installed mod {} version {} to {}",
                mod_to_download.name,
                mod_to_download.version,
                out_file.display()
            );
            Ok(())
        } else {
            error!(
                "Could not find mod on mod portal matching {}_{}",
                mod_to_download.name, mod_to_download.version
            );
            Err(Error::ModNotFound {
                mod_name: mod_to_download.name.clone(),
                mod_version: mod_to_download.version.clone(),
            })
        }
    }

    fn calculate_mod_delta(currently_installed: &[Mod], desired_state: &[Mod]) -> ModDelta {
        let mut mods_to_install: HashSet<Mod> = desired_state.iter().cloned().collect();
        let mut mods_to_delete = HashSet::new();

        for requested_mod in desired_state {
            if currently_installed.contains(&requested_mod) {
                mods_to_install.remove(&requested_mod);
            }
        }

        for existing_mod in currently_installed {
            if !desired_state.contains(&existing_mod) {
                mods_to_delete.insert(existing_mod.clone());
            }
        }

        ModDelta {
            install: mods_to_install,
            delete: mods_to_delete,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Mod {
    pub name: String,
    pub version: String,
}

impl Mod {
    fn try_from_filename(s: &str) -> Option<Mod> {
        // Per https://wiki.factorio.com/Tutorial:Mod_structure, mod zip files must be named with the pattern:
        // {mod-name}_{version-number}.zip
        // No support for unzipped mods (yet?)
        lazy_static! {
            static ref MOD_FILENAME_RE: Regex = Regex::new(r"^(.+)_(\d+\.\d+\.\d+)\.zip$").unwrap();
        }

        if let Some(captures) = MOD_FILENAME_RE.captures(s) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let version = captures.get(2).unwrap().as_str().to_string();
            Some(Mod { name, version })
        } else {
            debug!(
                "Filename {} could not be parsed into a mod name and version",
                s
            );
            None
        }
    }
}

struct ModDelta {
    install: HashSet<Mod>,
    delete: HashSet<Mod>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModList {
    mods: Vec<ModListElem>,
}

impl Default for ModList {
    fn default() -> Self {
        ModList {
            mods: vec![ModListElem {
                name: "base".to_owned(),
                enabled: true,
            }],
        }
    }
}

impl From<Vec<Mod>> for ModList {
    fn from(v: Vec<Mod>) -> Self {
        // Assume base is always enabled
        let mut elems = vec![ModListElem {
            name: "base".to_owned(),
            enabled: true,
        }];
        elems.extend(v.into_iter().map(|m| ModListElem {
            name: m.name,
            enabled: true,
        }));
        ModList { mods: elems }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModListElem {
    name: String,
    enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use fctrl::util;

    #[test]
    fn can_parse_valid_mod_filenames() -> std::result::Result<(), Box<dyn std::error::Error>> {
        util::testing::logger_init();

        let mut valid_names = HashMap::new();
        valid_names.insert(
            "A Sea Block Config_0.5.1.zip",
            Mod {
                name: "A Sea Block Config".to_owned(),
                version: "0.5.1".to_owned(),
            },
        );
        valid_names.insert(
            "AfraidOfTheDark_1.1.1.zip",
            Mod {
                name: "AfraidOfTheDark".to_owned(),
                version: "1.1.1".to_owned(),
            },
        );
        valid_names.insert(
            "Companion_Drones_1.0.19.zip",
            Mod {
                name: "Companion_Drones".to_owned(),
                version: "1.0.19".to_owned(),
            },
        );
        valid_names.insert(
            "KS_Power_quickfix_0.4.05.zip",
            Mod {
                name: "KS_Power_quickfix".to_owned(),
                version: "0.4.05".to_owned(),
            },
        );
        valid_names.insert(
            "Squeak Through_1.8.1.zip",
            Mod {
                name: "Squeak Through".to_owned(),
                version: "1.8.1".to_owned(),
            },
        );
        valid_names.insert(
            "Todo-List_19.1.0.zip",
            Mod {
                name: "Todo-List".to_owned(),
                version: "19.1.0".to_owned(),
            },
        );
        valid_names.insert(
            "train-pubsub_1.1.4.zip",
            Mod {
                name: "train-pubsub".to_owned(),
                version: "1.1.4".to_owned(),
            },
        );

        for (filename, expected) in valid_names {
            let parsed = Mod::try_from_filename(filename).unwrap();
            assert_eq!(parsed, expected);
        }

        Ok(())
    }

    #[test]
    fn cannot_parse_invalid_mod_filenames() -> std::result::Result<(), Box<dyn std::error::Error>> {
        util::testing::logger_init();

        let invalid_names = vec![
            "A Sea Block Config.zip",
            "AfraidOfTheDark_1.1.1.tar.gz",
            "Companion_Drones",
            "_1.8.1.zip",
            "19.1.0.zip",
            "train-pubsub_.zip",
        ];

        for name in invalid_names {
            let parsed = Mod::try_from_filename(name);
            assert!(parsed.is_none());
        }

        Ok(())
    }

    #[tokio::test]
    async fn can_query_mod_info() -> std::result::Result<(), Box<dyn std::error::Error>> {
        util::testing::logger_init();

        let mod_to_query = Mod {
            name: "rso-mod".to_owned(),
            version: "6.2.5".to_owned(),
        };

        assert!(ModManager::short_query_mod(&mod_to_query).await.is_ok());

        Ok(())
    }

    #[test]
    fn can_calculate_mod_delta_with_empty_current_list() {
        util::testing::logger_init();

        let current = vec![];
        let desired = vec![Mod {
            name: "rso-mod".to_owned(),
            version: "6.2.5".to_owned(),
        }];

        let delta = ModManager::calculate_mod_delta(&current, &desired);
        assert!(delta.delete.is_empty());
        assert_eq!(delta.install.into_iter().collect::<Vec<_>>(), desired);
    }

    #[test]
    fn can_calculate_mod_delta() {
        util::testing::logger_init();

        let current = vec![
            Mod {
                name: "test1".to_owned(),
                version: "2.3.4".to_owned(),
            },
            Mod {
                name: "test2".to_owned(),
                version: "1.2.5".to_owned(),
            },
            Mod {
                name: "rso-mod".to_owned(),
                version: "6.2.4".to_owned(),
            },
        ];
        let desired = vec![
            Mod {
                name: "test1".to_owned(),
                version: "2.3.4".to_owned(),
            },
            Mod {
                name: "rso-mod".to_owned(),
                version: "6.2.5".to_owned(),
            },
        ];

        let delta = ModManager::calculate_mod_delta(&current, &desired);
        assert_eq!(delta.delete.len(), 2);
        assert!(delta.delete.contains(&Mod {
            name: "test2".to_owned(),
            version: "1.2.5".to_owned(),
        }));
        assert!(delta.delete.contains(&Mod {
            name: "rso-mod".to_owned(),
            version: "6.2.4".to_owned(),
        }));
        assert_eq!(delta.install.len(), 1);
        assert_eq!(delta.install.len(), 1);
        assert!(delta.install.contains(&Mod {
            name: "rso-mod".to_owned(),
            version: "6.2.5".to_owned(),
        }));
    }
}
