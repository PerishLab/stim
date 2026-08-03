mod query;
mod schema;
mod write;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use crate::{Delivery, Message, Receipt, Replied, Reply, Staged, Synced};
use query::Query as _;
use schema::SCHEMA;
use write::{Write as _, now, staged, validate};

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
        participant: &str,
        soul: &str,
        request: &str,
        content: &str,
    ) -> Result<Staged, String> {
        validate(participant, soul, request, content)?;
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        if let Some(message) = tx.requested(request)? {
            if message.participant != participant
                || message.soul.as_deref() != Some(soul)
                || message.content != content
            {
                return Err(format!("request {request} conflicts with a staged message"));
            }
            return Ok(staged(message, soul, request));
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
            params![participant, soul, now],
        )
        .map_err(|error| error.to_string())?;
        let held: Option<String> = tx
            .query_row(
                "SELECT soul_id FROM conversations WHERE participant_id = ?1",
                [participant],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if held.as_deref() != Some(soul) {
            return Err(format!("participant {participant} belongs to another soul"));
        }
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        tx.execute(
            r#"
            INSERT INTO messages (
              id, participant_id, author, soul_id, request_id, content, created_at
            ) VALUES (?1, ?2, 'human', ?3, ?4, ?5, ?6)
            "#,
            params![id, participant, soul, request, content, now],
        )
        .map_err(|error| error.to_string())?;
        let message = tx
            .requested(request)?
            .ok_or_else(|| "staged message missing after insert".to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(staged(message, soul, request))
    }

    pub fn accept(&self, request: &str, receipt: &Receipt) -> Result<Message, String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let message = tx
            .requested(request)?
            .ok_or_else(|| format!("request {request} was not staged"))?;
        tx.bind(&message.participant, &receipt.strand)?;
        tx.execute(
            "UPDATE messages SET strand_id = ?1, receipt_id = ?2 WHERE request_id = ?3",
            params![receipt.strand, receipt.inbox, request],
        )
        .map_err(|error| error.to_string())?;
        let message = tx
            .requested(request)?
            .ok_or_else(|| "accepted message missing after update".to_string())?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(message)
    }

    pub fn poll(&self, participant: &str, since: i64) -> Result<Vec<Message>, String> {
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
            .query_map(params![participant, since.max(0)], query::mapped)
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    pub fn pending(&self) -> Result<Vec<Staged>, String> {
        self.conn
            .lock()
            .unwrap()
            .pending()?
            .into_iter()
            .map(|message| {
                let soul = message
                    .soul
                    .clone()
                    .ok_or_else(|| format!("pending message {} has no soul", message.id))?;
                let request = message
                    .request
                    .clone()
                    .ok_or_else(|| format!("pending message {} has no request", message.id))?;
                Ok(staged(message, &soul, &request))
            })
            .collect()
    }

    pub fn reply(&self, request: &Reply) -> Result<Replied, String> {
        if request.strand.trim().is_empty()
            || request.turn.trim().is_empty()
            || request.content.trim().is_empty()
        {
            return Err("strand, turn, and content must not be empty".to_string());
        }
        if request.strand.len() > 256 || request.turn.len() > 256 {
            return Err("strand or turn is too long".to_string());
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let held = tx.participant(&request.strand)?;
        if let Some(existing) = tx.turned(&request.turn)? {
            let reply = (
                held.as_deref(),
                existing.strand.as_deref(),
                existing.delivery.as_ref(),
                existing.content.as_str(),
            );
            let expected = (
                Some(existing.participant.as_str()),
                Some(request.strand.as_str()),
                Some(&Delivery::Explicit),
                request.content.as_str(),
            );
            if reply == expected {
                return Ok(Replied {
                    message: Some(existing),
                    pending: false,
                    deduplicated: true,
                });
            }
            return Err(format!(
                "turn {} conflicts with an existing reply",
                request.turn
            ));
        }
        if let Some((strand, content)) = tx.early(&request.turn)? {
            if strand == request.strand && content == request.content {
                return Ok(Replied {
                    message: None,
                    pending: true,
                    deduplicated: true,
                });
            }
            return Err(format!(
                "turn {} conflicts with a pending reply",
                request.turn
            ));
        }
        tx.defer(request)?;
        if let Some(participant) = held.as_deref() {
            tx.settle(participant, &request.strand)?;
        }
        let message = tx.turned(&request.turn)?;
        tx.commit().map_err(|error| error.to_string())?;
        Ok(Replied {
            message,
            pending: held.is_none(),
            deduplicated: false,
        })
    }

    pub fn sync(&self, batch: &crate::Events) -> Result<Synced, String> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction().map_err(|error| error.to_string())?;
        let current = tx.cursor()?;
        if batch.cursor < current {
            return Err("turn event cursor moved backwards".to_string());
        }
        let mut inserted = 0;
        for event in &batch.events {
            inserted += usize::from(tx.apply(event)?);
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
        self.conn.lock().unwrap().cursor()
    }
}
