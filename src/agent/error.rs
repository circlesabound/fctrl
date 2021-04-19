pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    // Process management
    ProcessAlreadyRunning,
    ProcessPidError,
    ProcessSignalError(nix::Error),

    // Generic wrappers around external error types
    Io(std::io::Error),
    Json(serde_json::error::Error),
    ModSettingsSerde(factorio_mod_settings_parser::Error),
    Reqwest(reqwest::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::error::Error> for Error {
    fn from(e: serde_json::error::Error) -> Self {
        Error::Json(e)
    }
}

impl From<factorio_mod_settings_parser::Error> for Error {
    fn from(e: factorio_mod_settings_parser::Error) -> Self {
        Error::ModSettingsSerde(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}

impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::TomlDe(e)
    }
}

impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Self {
        Error::TomlSer(e)
    }
}
