pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    // Process management
    ProcessAlreadyRunning,
    ProcessNotRunning,
    ProcessPidError,
    ProcessPipeError,
    ProcessSignalError(nix::Error),

    // Mods
    MalformedModList,
    ModNotFound {
        mod_name: String,
        mod_version: String,
    },

    // RCON
    RconEmptyCommand,
    RconNotConnected,

    // SaveHeader
    HeaderNotFound,

    // Generic
    Aggregate(Vec<Error>),

    // Generic wrappers around external error types
    FactorioDatFileSerde(factorio_file_parser::Error),
    Io(std::io::Error),
    Json(serde_json::error::Error),
    Rcon(rcon::Error),
    Reqwest(reqwest::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    Zip(async_zip::error::ZipError),
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

impl From<factorio_file_parser::Error> for Error {
    fn from(e: factorio_file_parser::Error) -> Self {
        Error::FactorioDatFileSerde(e)
    }
}

impl From<rcon::Error> for Error {
    fn from(e: rcon::Error) -> Self {
        Error::Rcon(e)
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

impl From<async_zip::error::ZipError> for Error {
    fn from(e: async_zip::error::ZipError) -> Self {
        Error::Zip(e)
    }
}
