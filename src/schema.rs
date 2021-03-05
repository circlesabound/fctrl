use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub console: Option<ConsoleOutConfig>,
    pub observers: Option<ObserversConfig>,
    pub rcon: Option<RconConfig>,
}

#[derive(Deserialize)]
pub struct AgentConfig {
    pub bind_address: String,
}

#[derive(Deserialize)]
pub struct ConsoleOutConfig {
    pub console_log_path: String,
}

#[derive(Deserialize)]
pub struct ObserversConfig {
    pub script_output_path: String,
}

#[derive(Deserialize)]
pub struct RconConfig {
    pub address: String,
    pub password: String,
}

#[derive(Deserialize)]
pub enum IncomingMessage {
    ChatPrint(String),
    RconCommand(String),
}

#[derive(Clone, Debug, Serialize)]
pub enum OutgoingMessage {
    ConsoleOut(ConsoleOutMessage)
}

#[derive(Clone, Debug, Serialize)]
pub enum ConsoleOutMessage {
    Chat { timestamp: String, user: String, msg: String },
    Join { timestamp: String, user: String },
    Leave { timestamp: String, user: String },
}
