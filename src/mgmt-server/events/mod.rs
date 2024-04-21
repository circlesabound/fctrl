use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use strum_macros::{AsRefStr, Display, EnumString};

pub mod broker;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Event {
    pub tags: HashMap<TopicName, String>,
    pub timestamp: DateTime<Utc>,
    pub content: String,
}

#[derive(
    Clone, Debug, Deserialize, Eq, derive_more::From, Hash, derive_more::Into, PartialEq, Serialize,
)]
pub struct TopicName {
    pub name: String,
}

impl TopicName {
    pub fn new(name: impl Into<String>) -> TopicName {
        TopicName {
            name: name.into(),
        }
    }
}

pub const OPERATION_TOPIC_NAME: &'static str =      "operation";
pub const STDOUT_TOPIC_NAME: &'static str =         "stdout";
pub const CHAT_TOPIC_NAME: &'static str =           "chat";
pub const JOIN_TOPIC_NAME: &'static str =           "join";
pub const LEAVE_TOPIC_NAME: &'static str =          "leave";
pub const RPC_TOPIC_NAME: &'static str =            "rpc";
pub const SERVERSTATE_TOPIC_NAME: &'static str =    "serverstate";

#[derive(EnumString, AsRefStr, Display)]
pub enum StdoutTopicCategory {
    #[strum(serialize = "chat")]
    Chat,
    #[strum(serialize = "chat_discord_echo")]
    ChatDiscordEcho,
    #[strum(serialize = "joinleave")]
    JoinLeave,
    #[strum(serialize = "rpc")]
    Rpc,
    #[strum(serialize = "serverstate")]
    ServerState,
    #[strum(serialize = "systemlog")]
    SystemLog,
}
