use serde::{Deserialize, Serialize};

pub const MARK: &str = "stim:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    Human,
    Soul,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Explicit,
    Automatic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub seq: i64,
    pub id: String,
    pub participant: String,
    pub author: Author,
    pub soul: Option<String>,
    pub strand: Option<String>,
    pub turn: Option<String>,
    pub request: Option<String>,
    pub receipt: Option<String>,
    pub delivery: Option<Delivery>,
    pub content: String,
    pub created: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ingest {
    pub soul: String,
    pub label: String,
    pub text: String,
    pub request: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    pub strand: String,
    pub inbox: String,
    pub warning: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Post {
    pub participant: String,
    pub soul: Option<String>,
    pub content: String,
    pub request: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posted {
    pub message: Message,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub strand: String,
    pub turn: String,
    pub label: String,
    pub text: String,
    pub completed: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Events {
    pub cursor: i64,
    pub events: Vec<Event>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub strand: String,
    pub turn: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replied {
    pub message: Option<Message>,
    pub pending: bool,
    pub deduplicated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub message: Message,
    pub request: Ingest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Synced {
    pub cursor: i64,
    pub inserted: usize,
}

pub fn label(participant: &str) -> String {
    format!("{MARK}{participant}")
}

pub fn participant(label: &str) -> Option<&str> {
    label.strip_prefix(MARK).filter(|value| !value.is_empty())
}
