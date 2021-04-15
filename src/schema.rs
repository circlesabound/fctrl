use serde::{Deserialize, Serialize};

// *************************
// * Configuration schemas *
// *************************

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub console: Option<ConsoleOutConfig>,
    pub observers: Option<ObserversConfig>,
    pub rcon: Option<RconConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AgentConfig {
    pub websocket_bind_address: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConsoleOutConfig {
    pub console_log_path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ObserversConfig {
    pub script_output_path: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RconConfig {
    pub address: String,
    pub password: String,
}

// *************************
// * WebSocket API schemas *
// *************************

#[derive(Clone, Debug, Deserialize, derive_more::From, derive_more::Into, Serialize)]
pub struct OperationId(String);

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentRequestWithId {
    pub operation_id: OperationId,
    pub message: AgentRequest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentRequest {
    // Installation management
    VersionInstall(String),

    // Server control
    ServerStart(ServerStartSaveFile),
    ServerStop,
    ServerStatus,

    // Save management
    SaveCreate(String),

    // In-game
    RconCommand(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentResponseWithId {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub content: AgentResponse,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum OperationStatus {
    Ongoing,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AgentResponse {
    // Generic responses
    Message(String),
    Error(String),
    Ok,

    // Structured messages
    ServerStatus(ServerStatus),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ServerStartSaveFile {
    Latest,
    Specific(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerStatus {
    pub running: bool,
}
