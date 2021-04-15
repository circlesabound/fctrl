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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OperationId(String);

#[derive(Debug, Deserialize)]
pub struct IncomingMessage {
    pub operation_id: OperationId,
    pub request: IncomingRequest,
}

#[derive(Clone, Debug, Deserialize)]
pub enum IncomingRequest {
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

#[derive(Clone, Debug, Serialize)]
pub struct OutgoingMessageWithId {
    pub operation_id: OperationId,
    pub status: OperationStatus,
    pub content: OutgoingMessage,
}

#[derive(Clone, Debug, Serialize)]
pub enum OperationStatus {
    Ongoing,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub enum OutgoingMessage {
    Message(String),
    Error(String),
    Ok,
}

#[derive(Clone, Debug, Deserialize)]
pub enum ServerStartSaveFile {
    Latest,
    Specific(String),
}
