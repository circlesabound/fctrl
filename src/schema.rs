use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ******************************************
// * mgmt-server REST API schemas           *
// * autogenerated by openapi-generator-cli *
// ******************************************

pub mod mgmt_server_rest {
    include!(concat!(env!("OUT_DIR"), "/mgmt-server-rest.rs"));
}

// *******************************************
// * Factorio Mod Portal API schemas         *
// * autogenerated by openapi-generator-cli  *
// *******************************************

pub mod factorio_mod_portal_api {
    include!(concat!(env!("OUT_DIR"), "/factorio-mod-portal.rs"));
}

// *******************************************
// * WebSocket API schemas                   *
// *******************************************

#[derive(Clone, Debug, Deserialize, derive_more::From, derive_more::Into, Serialize)]
pub struct OperationId(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentRequestWithId {
    pub operation_id: OperationId,
    pub message: AgentRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentRequest {
    // *********************************
    // * Installation management       *
    // *********************************
    //
    //
    /// Install the requested version, overwriting the existing installation if different version.
    /// Can specify the force_install flag to force a re-install of the current version.
    ///
    /// **This is a long-running operation.**
    VersionInstall {
        version: String,
        force_install: bool,
    },
    /// Get the currently installed version, if any.
    VersionGet,

    // *********************************
    // * Server control                *
    // *********************************
    //
    //
    /// Start the server using the specific save file.
    ServerStart(ServerStartSaveFile),
    /// Stop the server.
    ServerStop,
    /// Get the current status of the server.
    ServerStatus,

    // *********************************
    // * Save management               *
    // *********************************
    //
    //
    /// Create a save file with the requested name.
    /// This will overwrite any existing save file of that name.
    ///
    /// **This is a long-running operation.**
    SaveCreate(String),
    /// Get a list of the save files present on the server.
    SaveList,

    // *********************************
    // * Mod management                *
    // *********************************
    //
    //
    /// Get a list of mods installed on the server.
    ModListGet,
    /// Applies the desired mod list on the server.
    ///
    /// **This is a long-running operation.**
    ModListSet(Vec<ModObject>),
    /// Gets the mod-settings file on the server.
    ModSettingsGet,
    /// Sets the mod-settings file on the servere.
    ModSettingsSet(Vec<u8>),

    // *********************************
    // * Configuration                 *
    // *********************************
    //
    //
    /// Gets a list of users with admin privileges on the server.
    ConfigAdminListGet,
    /// Sets the list of users with admin privileges on the server.
    ConfigAdminListSet {
        admins: Vec<String>,
    },
    ConfigRconGet,
    ConfigRconSet {
        password: String,
    },
    ConfigSecretsGet,
    ConfigSecretsSet {
        username: String,
        token: String,
    },
    ConfigServerSettingsGet,
    ConfigServerSettingsSet {
        json: String,
    },

    // *********************************
    // * In-game                       *
    // *********************************
    RconCommand(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentResponseWithId {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub content: AgentOutMessage,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum OperationStatus {
    /// Indicates a long running operation, and that there will be future responses on the
    /// same operation id.
    Ack,

    /// Indicates some measure of progress on a long running operation.
    Ongoing,

    /// Indicates that the operation, whether long-running or short-running, has completed
    /// successfully, and no futher responses on the same operation id are expected.
    Completed,

    /// Indicates that the operation, whether long-running or short-running, has failed,
    /// and no futher responses on the same operation id are expected.
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentOutMessage {
    // Generic responses
    Message(String),
    Error(String),
    Ok,

    // Structured operation responses
    ConflictingOperation,
    ConfigAdminList(Vec<String>),
    ConfigRcon { port: u16, password: String },
    ConfigSecrets(Option<SecretsObject>),
    ConfigServerSettings(String),
    FactorioVersion(FactorioVersion),
    ModsList(Vec<ModObject>),
    ModSettings(Option<Vec<u8>>),
    MissingSecrets,
    NotInstalled,
    RconResponse(String),
    SaveList(Vec<Save>),
    ServerStatus(ServerStatus),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ServerStartSaveFile {
    Latest,
    Specific(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ServerStatus {
    NotRunning,
    PreGame,
    InGame { player_count: u32 },
    PostGame,
}

#[derive(Clone, Debug, Deserialize, derive_more::From, derive_more::Into, Serialize)]
pub struct FactorioVersion(String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Save {
    pub name: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModObject {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecretsObject {
    pub username: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentStreamingMessage {
    ServerStdout(String),
}
