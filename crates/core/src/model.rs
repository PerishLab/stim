use serde::{Deserialize, Serialize};

pub const LABEL_PREFIX: &str = "stim:";

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
    pub participant_id: String,
    pub author: Author,
    pub soul_id: Option<String>,
    pub strand_id: Option<String>,
    pub turn_id: Option<String>,
    pub request_id: Option<String>,
    pub receipt_id: Option<String>,
    pub delivery: Option<Delivery>,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestRequest {
    pub soul_id: String,
    pub label: String,
    pub text: String,
    pub request_id: String,
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestReceipt {
    pub strand_id: String,
    pub inbox_id: String,
    pub warning: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRequest {
    pub participant_id: String,
    pub soul_id: Option<String>,
    pub content: String,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: Message,
    pub receipt: IngestReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEvent {
    pub id: String,
    pub strand_id: String,
    pub turn_id: String,
    pub external_label: String,
    pub final_text: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEventBatch {
    pub cursor: i64,
    pub events: Vec<TurnEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyRequest {
    pub strand_id: String,
    pub turn_id: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplyResponse {
    pub message: Option<Message>,
    pub pending: bool,
    pub deduplicated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staged {
    pub message: Message,
    pub request: IngestRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Synced {
    pub cursor: i64,
    pub inserted: usize,
}

pub fn label(participant: &str) -> String {
    format!("{LABEL_PREFIX}{participant}")
}

pub fn participant(label: &str) -> Option<&str> {
    label
        .strip_prefix(LABEL_PREFIX)
        .filter(|value| !value.is_empty())
}
