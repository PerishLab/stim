use chrono::Utc;
use rusqlite::{Transaction, params};

use crate::{Delivery, Event, Message, Reply, Staged, label, participant};

use super::query::Query;

pub(super) struct Penned<'a> {
    pub participant: &'a str,
    pub strand: &'a str,
    pub turn: &'a str,
    pub content: &'a str,
    pub delivery: Delivery,
    pub created: &'a str,
}

pub(super) fn staged(message: Message, soul: &str, request: &str) -> Staged {
    Staged {
        request: crate::Ingest {
            soul: soul.to_string(),
            label: label(&message.participant),
            text: message.content.clone(),
            request: request.to_string(),
            source: Some(format!("stim:{request}")),
        },
        message,
    }
}

pub(super) fn validate(
    participant: &str,
    soul: &str,
    request: &str,
    content: &str,
) -> Result<(), String> {
    if participant.trim().is_empty() || soul.trim().is_empty() || request.trim().is_empty() {
        return Err("participant, soul, and request must not be empty".to_string());
    }
    if participant.len() > 128 || soul.len() > 256 || request.len() > 256 {
        return Err("participant, soul, or request is too long".to_string());
    }
    if content.trim().is_empty() {
        return Err("content must not be empty".to_string());
    }
    Ok(())
}

pub(super) trait Write {
    fn bind(&self, participant: &str, strand: &str) -> Result<(), String>;
    fn apply(&self, event: &Event) -> Result<bool, String>;
    fn defer(&self, request: &Reply) -> Result<(), String>;
    fn settle(&self, participant: &str, strand: &str) -> Result<(), String>;
    fn penned(&self, note: &Penned<'_>) -> Result<Message, String>;
}

impl Write for Transaction<'_> {
    fn bind(&self, participant: &str, strand: &str) -> Result<(), String> {
        let existing = self.participant(strand)?;
        if existing
            .as_deref()
            .is_some_and(|value| value != participant)
        {
            return Err(format!("strand {strand} belongs to another participant"));
        }
        let current: Option<String> = self
            .query_row(
                "SELECT strand_id FROM conversations WHERE participant_id = ?1",
                [participant],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if current.as_deref().is_some_and(|value| value != strand) {
            return Err(format!(
                "participant {participant} belongs to another strand"
            ));
        }
        self.execute(
            "UPDATE conversations SET strand_id = ?1, updated_at = ?2 WHERE participant_id = ?3",
            params![strand, now(), participant],
        )
        .map_err(|error| error.to_string())?;
        self.settle(participant, strand)?;
        Ok(())
    }

    fn apply(&self, event: &Event) -> Result<bool, String> {
        let held = participant(&event.label)
            .ok_or_else(|| format!("event {} is outside the stim zone", event.id))?;
        let now = now();
        self.execute(
            r#"
            INSERT INTO conversations (participant_id, strand_id, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?3)
            ON CONFLICT(participant_id) DO NOTHING
            "#,
            params![held, event.strand, now],
        )
        .map_err(|error| error.to_string())?;
        self.bind(held, &event.strand)?;
        if let Some(existing) = self.turned(&event.turn)? {
            let origin = (existing.participant.as_str(), existing.strand.as_deref());
            let expected = (held, Some(event.strand.as_str()));
            let admitted = existing.delivery.as_ref() == Some(&Delivery::Explicit)
                || existing.content == event.text;
            if origin == expected && admitted {
                return Ok(false);
            }
            return Err(format!(
                "turn {} conflicts with an existing reply",
                event.turn
            ));
        }
        self.penned(&Penned {
            participant: held,
            strand: &event.strand,
            turn: &event.turn,
            content: &event.text,
            delivery: Delivery::Automatic,
            created: &event.completed,
        })?;
        Ok(true)
    }

    fn defer(&self, request: &Reply) -> Result<(), String> {
        self.execute(
            r#"
            INSERT INTO early_replies (turn_id, strand_id, content, created_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![request.turn, request.strand, request.content, now()],
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn settle(&self, participant: &str, strand: &str) -> Result<(), String> {
        let mut stmt = self
            .prepare(
                r#"
                SELECT turn_id, content, created_at
                FROM early_replies WHERE strand_id = ?1 ORDER BY created_at
                "#,
            )
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([strand], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        drop(stmt);
        for (turn, content, created) in rows {
            if self.turned(&turn)?.is_none() {
                self.penned(&Penned {
                    participant,
                    strand,
                    turn: &turn,
                    content: &content,
                    delivery: Delivery::Explicit,
                    created: &created,
                })?;
            }
            self.execute("DELETE FROM early_replies WHERE turn_id = ?1", [&turn])
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn penned(&self, note: &Penned<'_>) -> Result<Message, String> {
        let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
        self.execute(
            r#"
            INSERT INTO messages (
              id, participant_id, author, strand_id, turn_id, delivery, content, created_at
            ) VALUES (?1, ?2, 'soul', ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                id,
                note.participant,
                note.strand,
                note.turn,
                coded(note.delivery),
                note.content,
                note.created
            ],
        )
        .map_err(|error| error.to_string())?;
        self.turned(note.turn)?
            .ok_or_else(|| "reply missing after insert".to_string())
    }
}

fn coded(value: Delivery) -> &'static str {
    match value {
        Delivery::Explicit => "explicit",
        Delivery::Automatic => "automatic",
    }
}

pub(super) fn now() -> String {
    Utc::now().to_rfc3339()
}
