use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, EnumString};

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

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentRequestWithId {
    pub operation_id: OperationId,
    pub message: AgentRequest,
}

#[derive(Debug, Deserialize, Serialize)]
pub enum AgentRequest {
    // *********************************
    // * Internal build information    *
    // *********************************
    //
    //
    /// Get the build info for the agent
    BuildVersion,

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
        version: FactorioVersion,
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
    /// Create a new save file with the requested name.
    /// This will overwrite any existing save file of that name.
    ///
    /// **This is a long-running operation.**
    SaveCreate(String),
    /// Gets the save file zip from the server
    SaveGet(String),
    /// Get a list of the save files present on the server.
    SaveList,
    /// Upserts a save file with the requested name
    SaveSet(String, SaveBytes),

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
    ModSettingsSet(ModSettingsBytes),

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
    ConfigBanListGet,
    ConfigBanListSet {
        users: Vec<String>,
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
        config: ServerSettingsConfig,
    },
    ConfigWhiteListGet,
    ConfigWhiteListSet {
        enabled: bool,
        users: Vec<String>,
    },

    // *********************************
    // * In-game                       *
    // *********************************
    RconCommand(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentResponseWithId {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub timestamp: DateTime<Utc>,
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

#[derive(Debug, Deserialize, Serialize)]
pub enum AgentOutMessage {
    // Generic responses
    Message(String),
    Error(String),
    Ok,

    // Structured operation responses
    AgentBuildVersion(BuildVersion),
    ConflictingOperation,
    ConfigAdminList(Vec<String>),
    ConfigBanList(Vec<String>),
    ConfigWhiteList(WhitelistObject),
    ConfigRcon(RconConfig),
    ConfigSecrets(Option<SecretsObject>),
    ConfigServerSettings(ServerSettingsConfig),
    FactorioVersion(FactorioVersion),
    ModsList(Vec<ModObject>),
    ModSettings(Option<ModSettingsBytes>),
    MissingSecrets,
    NotInstalled,
    RconResponse(String),
    SaveFile(SaveBytes),
    SaveList(Vec<Save>),
    SaveNotFound,
    ServerStatus(ServerStatus),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildVersion {
    pub timestamp: String,
    pub commit_hash: String,
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
pub struct FactorioVersion(pub String);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Save {
    pub name: String,
    pub last_modified: DateTime<Utc>,
}

#[derive(Deserialize, Serialize)]
pub struct SaveBytes {
    pub multipart_seqnum: Option<u32>,
    #[serde(with = "base64")]
    pub bytes: Vec<u8>,
}

impl SaveBytes {
    pub fn new(bytes: Vec<u8>) -> SaveBytes {
        SaveBytes {
            multipart_seqnum: None,
            bytes,
        }
    }

    pub fn sentinel(total_num_parts: u32) -> SaveBytes {
        SaveBytes {
            multipart_seqnum: Some(total_num_parts),
            bytes: vec![],
        }
    }
}

impl std::fmt::Debug for SaveBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.bytes.len() > 16 {
            let debug_bytes = format!("{:?}...", &self.bytes[..16]);
            f.debug_struct("SaveBytes")
                .field("multipart_seqnum", &self.multipart_seqnum)
                .field("bytes", &debug_bytes)
                .finish()
        } else {
            f.debug_struct("SaveBytes")
                .field("multipart_seqnum", &self.multipart_seqnum)
                .field("bytes", &self.bytes)
                .finish()
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ModSettingsBytes {
    #[serde(with = "base64")]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModObject {
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RconConfig {
    pub port: u16,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SecretsObject {
    pub username: String,
    pub token: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WhitelistObject {
    pub enabled: bool,
    pub users: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentStreamingMessage {
    pub timestamp: DateTime<Utc>,
    pub content: AgentStreamingMessageInner,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentStreamingMessageInner {
    ServerStdout(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerSettingsConfig {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub visibility: ServerVisibilityConfig,

    pub autosave_interval: u32,
    pub autosave_only_on_server: bool,
    pub non_blocking_saving: bool,

    pub game_password: String,
    pub require_user_verification: bool,
    pub max_players: u32,
    pub ignore_player_limit_for_returning_players: bool,

    pub allow_commands: AllowCommandsValue,
    pub only_admins_can_pause_the_game: bool,

    pub max_upload_in_kilobytes_per_second: u32,
    pub max_upload_slots: u32,

    pub minimum_latency_in_ticks: u32,
    pub max_heartbeats_per_second: u32,
    pub minimum_segment_size: u32,
    pub minimum_segment_size_peer_count: u32,
    pub maximum_segment_size: u32,
    pub maximum_segment_size_peer_count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerVisibilityConfig {
    pub public: bool,
    pub lan: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AllowCommandsValue {
    #[serde(rename = "true")]
    True,
    #[serde(rename = "false")]
    False,
    #[serde(rename = "admins-only")]
    AdminsOnly,
}

/// Internal state of the Factorio multiplayer server as tracked by output logs
#[derive(Clone, Debug, EnumString, AsRefStr)]
pub enum InternalServerState {
    Ready,
    PreparedToHostGame,
    CreatingGame,
    InGame,
    InGameSavingMap,
    DisconnectingScheduled,
    Disconnecting,
    Disconnected,
    Closed,
}

/// module for serde to handle binary fields
mod base64 {
    use base64::Engine;
    use serde::{Deserialize, Serialize};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Vec<u8>, s: S) -> Result<S::Ok, S::Error> {
        let base64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(v);
        String::serialize(&base64, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let base64 = String::deserialize(d)?;
        base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(base64.as_bytes())
            .map_err(|e| serde::de::Error::custom(e))
    }
}

pub mod regex {
    use lazy_static::lazy_static;
    use regex::Regex;

    lazy_static! {
        // echo from achievement-preserve setting discord chat link
        pub static ref CHAT_DISCORD_ECHO_RE: Regex = Regex::new(
            r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] <server>: \[Discord\] (.+)$"
        ).unwrap();
        // chat message from process stdout
        pub static ref CHAT_RE: Regex = Regex::new(
            r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[CHAT\] ([^:]+): (.+)$"
        ).unwrap();
        // player join event from process stdout
        pub static ref JOIN_RE: Regex = Regex::new(
            r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[JOIN\] ([^:]+) joined the game$"
        ).unwrap();
        // player leave event from process stdout
        pub static ref LEAVE_RE: Regex = Regex::new(
            r"^(\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) \[LEAVE\] ([^:]+) left the game$"
        ).unwrap();
        pub static ref MOD_FILENAME_RE: Regex = Regex::new(
            r"^(.+)_(\d+\.\d+\.\d+)\.zip$"
        ).unwrap();
        // RCON interface up event from process stdout
        pub static ref RCON_READY_RE: Regex = Regex::new(
            r"Starting RCON interface at IP ADDR:\(\{\d+\.\d+\.\d+\.\d+:(\d+)\}\)"
        ).unwrap();
        // FCTRL_RPC event from process stdout
        pub static ref RPC_RE: Regex = Regex::new(
            r"^FCTRL_RPC (.+)$"
        ).unwrap();
        // server internal state change from process stdout
        pub static ref STATE_CHANGE_RE: Regex = Regex::new(
            r"changing state from\(([a-zA-Z]+)\) to\(([a-zA-Z]+)\)"
        ).unwrap();
    }
}
