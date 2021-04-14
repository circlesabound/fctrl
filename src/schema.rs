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

#[derive(Clone, Debug, Deserialize)]
pub enum IncomingMessage {
    // Installation management
    VersionInstall(String),

    // Server control
    ServerStart(ServerStartSaveFile),
    ServerStop,
    ServerStatus,

    // Save management
    SaveCreate(String),

    // In-game
    ChatPrint(String),
    RconCommand(String),
}

#[derive(Clone, Debug, Serialize)]
pub enum OutgoingMessage {
    ConsoleOut(ConsoleOutMessage),
}

#[derive(Clone, Debug, Serialize)]
pub enum ConsoleOutMessage {
    Chat {
        timestamp: String,
        user: String,
        msg: String,
    },
    Join {
        timestamp: String,
        user: String,
    },
    Leave {
        timestamp: String,
        user: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub enum ServerStartSaveFile {
    Latest,
    Specific(String),
}
