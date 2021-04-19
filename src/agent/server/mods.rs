use std::{collections::HashMap, convert::{TryFrom, TryInto}, io::ErrorKind, path::PathBuf};

use factorio_mod_settings_parser::ModSettings;
use lazy_static::lazy_static;
use log::{debug, error, info};
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
            // We expect at least mod list here
            match fs::read_to_string(&*MOD_LIST_PATH).await {
                Ok(s) => {
                    let mods;
                    match serde_json::from_str::<ModList>(&s) {
                        Ok(ml) => {
                            // Get mod names except base
                            let mut mod_name_version_map = HashMap::new();
                            for elem in ml.mods {
                                if elem.name != "base" {
                                    mod_name_version_map.insert(elem.name, None);
                                }
                            }

                            // Need to get the versions from the dir entries
                            let mut entries = fs::read_dir(&*MOD_DIR).await?;
                            while let Some(entry) = entries.next_entry().await? {
                                if let Some(base_name) = entry.path().file_stem() {
                                    let base_name = base_name.to_str().unwrap().to_string();
                                    let split: Vec<_> = base_name.rsplitn(2, '_').collect();
                                    if split.len() == 2 {
                                        let mod_name = split[1];
                                        let version = split[0];
                                        if let Some(v) = mod_name_version_map.get_mut(mod_name) {
                                            debug!("matched version {} for mod {}", version, mod_name);
                                            v.replace(version.to_string());
                                        }
                                    }
                                }
                            }

                            let mut missed = false;
                            for (name, ..) in mod_name_version_map.iter().filter(|(_, v)| v.is_none()) {
                                error!("Missing version match in mod dir for mod {}", name);
                                missed = true;
                            }
                            if missed {
                                return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Missing version match file in mod dir").into());
                            } else {
                                mods = mod_name_version_map.into_iter().map(|(n, v)| Mod {
                                    name: n,
                                    version: v.unwrap(),
                                }).collect();
                            }
                        }
                        Err(e) => {
                            error!("Error parsing mod list: {:?}", e);
                            return Err(e.into());
                        }
                    }

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
                Err(e) => {
                    if e.kind() == ErrorKind::NotFound {
                        // mod-list.json missing
                        Ok(None)
                    } else {
                        error!("Error reading mod list: {:?}", e);
                        Err(e.into())
                    }
                }
            }
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
                ret.write().await?;
                Ok(ret)
            }
        }
    }

    pub async fn write(&self) -> Result<()> {
        fs::create_dir_all(&*MOD_DIR).await?;

        let mod_list_json = serde_json::to_string(&ModList::from(self.mods.clone()))?;
        fs::write(&*MOD_LIST_PATH, mod_list_json).await?;

        if let Some(settings) = self.settings.clone() {
            let bytes: Vec<u8> = settings.try_into()?;
            fs::write(&*MOD_SETTINGS_PATH, bytes).await?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Mod {
    pub name: String,
    pub version: String,
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
            }]
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
        ModList {
            mods: elems,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ModListElem {
    name: String,
    enabled: bool,
}
