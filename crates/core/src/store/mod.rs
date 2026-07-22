mod query;
mod schema;
mod write;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use crate::{Delivery, IngestReceipt, Message, ReplyRequest, ReplyResponse, Staged, Synced};
use query::{cursor, early_by_turn, message_by_request, message_by_turn, participant_for_strand};
use schema::SCHEMA;
use write::{apply, bind, defer, materialize, now, staged, validate};

#[derive(Clone)]
pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let conn = Connection::open(path).map_err(|error| error.to_string())?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        conn.pragma_update(None, "foreign_keys", true)
            .map_err(|error| error.to_string())?;
        conn.execute_batch(SCHEMA)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn stage(
        &self,
        participant_id: &str,
        soul_id: &str,
        request_id: &str,
        content: &str,
    ) -> Result<Staged, String> {
        validate(participant_id, soul_id, request_id, content)?;
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        if let Some(message) = message_by_request(&tx, request_id)? {
            if message.participant_id != participant_id
                || message.soul_id.as_deref() != Some(soul_id)
                || message.content != content
            {
                return Err(format!(
                    "request {request_id} conflicts with a staged message"
                ));
            }
            return Ok(staged(message, soul_id, request_id));
        }
        let now = now();
        tx.execute(
            r#"
            INSERT INTO conversations (participant_id, soul_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(participant_id) DO UPDATE SET
              soul_id = COALESCE(conversations.soul_id, excluded.soul_id),
              updated_at = excluded.updated_at
            "#,
            params![participant_id, soul_id, now],
        )
        .map_err(|error| error.to_string())?;
        let existing_soul: Option<String> = tx
            .query_row(
                "SELECT soul_id FROM conversations WHERE participant_id = ?1",
                [participant_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if existing_soul.as_deref() != Some(soul_id) {
            return Err(format!(
                "participant {participant_id} belongs to another soul"
            ));
        }
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        tx.execute(
            r#"
            INSERT INTO messages (
              id, participant_id, author, soul_id, request_id, content, created_at
            ) VALUES (?1, ?2, 'human', ?3, ?4, ?5, ?6)
            "#,
            params![id, participant_id, soul_id, request_id, content, now],
        )
        .map_err(|error| error.to_string())?;
        let message = message_by_request(&tx, request_id)?
            .ok_or_else(|| "staged message missing after insert".to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(staged(message, soul_id, request_id))
    }

    pub fn accept(&self, request_id: &str, receipt: &IngestReceipt) -> Result<Message, String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let message = message_by_request(&tx, request_id)?
            .ok_or_else(|| format!("request {request_id} was not staged"))?;
        bind(&tx, &message.participant_id, &receipt.strand_id)?;
        tx.execute(
            "UPDATE messages SET strand_id = ?1, receipt_id = ?2 WHERE request_id = ?3",
            params![receipt.strand_id, receipt.inbox_id, request_id],
        )
        .map_err(|error| error.to_string())?;
        let message = message_by_request(&tx, request_id)?
            .ok_or_else(|| "accepted message missing after update".to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(message)
    }

    pub fn poll(&self, participant_id: &str, since: i64) -> Result<Vec<Message>, String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                r#"
                SELECT seq, id, participant_id, author, soul_id, strand_id, turn_id,
                       request_id, receipt_id, delivery, content, created_at
                FROM messages
                WHERE participant_id = ?1 AND seq > ?2
                ORDER BY seq ASC
                "#,
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map(params![participant_id, since.max(0)], query::map_message)
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn pending(&self) -> Result<Vec<Staged>, String> {
        query::pending(&self.conn.lock().unwrap())?
            .into_iter()
            .map(|message| {
                let soul = message
                    .soul_id
                    .clone()
                    .ok_or_else(|| format!("pending message {} has no soul", message.id))?;
                let request = message
                    .request_id
                    .clone()
                    .ok_or_else(|| format!("pending message {} has no request", message.id))?;
                Ok(staged(message, &soul, &request))
            })
            .collect()
    }

    pub fn reply(&self, request: &ReplyRequest) -> Result<ReplyResponse, String> {
        if request.strand_id.trim().is_empty()
            || request.turn_id.trim().is_empty()
            || request.content.trim().is_empty()
        {
            return Err("strand_id, turn_id, and content must not be empty".to_string());
        }
        if request.strand_id.len() > 256 || request.turn_id.len() > 256 {
            return Err("strand_id or turn_id is too long".to_string());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let participant_id = participant_for_strand(&tx, &request.strand_id)?;
        if let Some(existing) = message_by_turn(&tx, &request.turn_id)? {
            if participant_id.as_deref() == Some(existing.participant_id.as_str())
                && existing.strand_id.as_deref() == Some(request.strand_id.as_str())
                && existing.delivery == Some(Delivery::Explicit)
                && existing.content == request.content
            {
                return Ok(ReplyResponse {
                    message: Some(existing),
                    pending: false,
                    deduplicated: true,
                });
            }
            return Err(format!(
                "turn {} conflicts with an existing reply",
                request.turn_id
            ));
        }
        if let Some((strand, content)) = early_by_turn(&tx, &request.turn_id)? {
            if strand == request.strand_id && content == request.content {
                return Ok(ReplyResponse {
                    message: None,
                    pending: true,
                    deduplicated: true,
                });
            }
            return Err(format!(
                "turn {} conflicts with a pending reply",
                request.turn_id
            ));
        }
        defer(&tx, request)?;
        if let Some(participant_id) = participant_id.as_deref() {
            materialize(&tx, participant_id, &request.strand_id)?;
        }
        let message = message_by_turn(&tx, &request.turn_id)?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(ReplyResponse {
            message,
            pending: participant_id.is_none(),
            deduplicated: false,
        })
    }

    pub fn sync(&self, batch: &crate::TurnEventBatch) -> Result<Synced, String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let current = cursor(&tx)?;
        if batch.cursor < current {
            return Err("turn event cursor moved backwards".to_string());
        }
        let mut inserted = 0;
        for event in &batch.events {
            inserted += usize::from(apply(&tx, event)?);
        }
        tx.execute(
            "UPDATE state SET value = ?1 WHERE key = 'santi_cursor'",
            [batch.cursor],
        )
        .map_err(|error| error.to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(Synced {
            cursor: batch.cursor,
            inserted,
        })
    }

    pub fn cursor(&self) -> Result<i64, String> {
        cursor(&self.conn.lock().unwrap())
    }
}
