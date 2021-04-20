use std::{
    convert::{TryFrom, TryInto},
    path::PathBuf,
};

use factorio_mod_settings_parser::ModSettings;
use lazy_static::lazy_static;
use log::{debug, error, info};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::{consts::*, error::Result};

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
                if path != *MOD_SETTINGS_PATH {
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
                path: MOD_SETTINGS_PATH.clone(),
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
                ret.apply().await?;
                Ok(ret)
            }
        }
    }

    pub async fn apply(&self) -> Result<()> {
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
}

#[derive(Clone, Debug, PartialEq)]
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
}
