use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
pub struct TopicName(pub String);

pub const OPERATION_TOPIC_NAME: &str = "operation";
pub const STDOUT_TOPIC_NAME: &str = "stdout";
pub const STDOUT_TOPIC_CHAT_CATEGORY: &str = "chat";
pub const STDOUT_TOPIC_CHAT_DISCORD_ECHO_CATEGORY: &str = "chat_discord_echo";
pub const STDOUT_TOPIC_JOINLEAVE_CATEGORY: &str = "joinleave";
pub const STDOUT_TOPIC_RPC: &str = "rpc";
pub const STDOUT_TOPIC_SYSTEMLOG_CATEGORY: &str = "systemlog";

pub const CHAT_TOPIC_NAME: &str = "chat";
pub const JOIN_TOPIC_NAME: &str = "join";
pub const LEAVE_TOPIC_NAME: &str = "leave";
pub const RPC_TOPIC_NAME: &str = "rpc";
