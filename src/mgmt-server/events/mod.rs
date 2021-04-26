use std::collections::HashMap;

pub mod broker;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event {
    pub tags: HashMap<TopicName, String>,
    pub content: String,
}

#[derive(Clone, Debug, Eq, derive_more::From, Hash, derive_more::Into, PartialEq)]
pub struct TopicName(pub String);

pub const OPERATION_TOPIC_NAME: &'static str = "operation";
pub const STDOUT_TOPIC_NAME: &'static str = "stdout";
