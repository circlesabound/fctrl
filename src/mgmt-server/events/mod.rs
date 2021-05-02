use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub mod broker;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Event {
    pub tags: HashMap<TopicName, String>,
    pub content: String,
}

#[derive(
    Clone, Debug, Deserialize, Eq, derive_more::From, Hash, derive_more::Into, PartialEq, Serialize,
)]
pub struct TopicName(pub String);

pub const OPERATION_TOPIC_NAME: &str = "operation";
pub const STDOUT_TOPIC_NAME: &str = "stdout";
