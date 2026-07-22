use chrono::Utc;
use rusqlite::{Transaction, params};

use crate::{Delivery, Message, Staged, TurnEvent, label, participant};

use super::query::{message_by_turn, participant_for_strand};

pub(super) fn staged(message: Message, soul_id: &str, request_id: &str) -> Staged {
    Staged {
        request: crate::IngestRequest {
            soul_id: soul_id.to_string(),
            label: label(&message.participant_id),
            text: message.content.clone(),
            request_id: request_id.to_string(),
            source_ref: Some(format!("stim:{request_id}")),
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
        return Err("participant_id, soul_id, and request_id must not be empty".to_string());
    }
    if participant.len() > 128 || soul.len() > 256 || request.len() > 256 {
        return Err("participant_id, soul_id, or request_id is too long".to_string());
    }
    if content.trim().is_empty() {
        return Err("content must not be empty".to_string());
    }
    Ok(())
}

pub(super) fn bind(tx: &Transaction<'_>, participant: &str, strand: &str) -> Result<(), String> {
    let existing = participant_for_strand(tx, strand)?;
    if existing
        .as_deref()
        .is_some_and(|value| value != participant)
    {
        return Err(format!("strand {strand} belongs to another participant"));
    }
    let current: Option<String> = tx
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
    tx.execute(
        "UPDATE conversations SET strand_id = ?1, updated_at = ?2 WHERE participant_id = ?3",
        params![strand, now(), participant],
    )
    .map_err(|error| error.to_string())?;
    materialize(tx, participant, strand)?;
    Ok(())
}

pub(super) fn apply(tx: &Transaction<'_>, event: &TurnEvent) -> Result<bool, String> {
    let participant_id = participant(&event.external_label)
        .ok_or_else(|| format!("event {} is outside the stim zone", event.id))?;
    let now = now();
    tx.execute(
        r#"
        INSERT INTO conversations (participant_id, strand_id, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?3)
        ON CONFLICT(participant_id) DO NOTHING
        "#,
        params![participant_id, event.strand_id, now],
    )
    .map_err(|error| error.to_string())?;
    bind(tx, participant_id, &event.strand_id)?;
    if let Some(existing) = message_by_turn(tx, &event.turn_id)? {
        if existing.participant_id == participant_id
            && existing.strand_id.as_deref() == Some(event.strand_id.as_str())
            && (existing.delivery == Some(Delivery::Explicit)
                || existing.content == event.final_text)
        {
            return Ok(false);
        }
        return Err(format!(
            "turn {} conflicts with an existing reply",
            event.turn_id
        ));
    }
    insert_reply(
        tx,
        participant_id,
        &event.strand_id,
        &event.turn_id,
        &event.final_text,
        Delivery::Automatic,
        &event.completed_at,
    )?;
    Ok(true)
}

pub(super) fn defer(tx: &Transaction<'_>, request: &crate::ReplyRequest) -> Result<(), String> {
    tx.execute(
        r#"
        INSERT INTO early_replies (turn_id, strand_id, content, created_at)
        VALUES (?1, ?2, ?3, ?4)
        "#,
        params![request.turn_id, request.strand_id, request.content, now()],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn materialize(
    tx: &Transaction<'_>,
    participant: &str,
    strand: &str,
) -> Result<(), String> {
    let mut stmt = tx
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
        if message_by_turn(tx, &turn)?.is_none() {
            insert_reply(
                tx,
                participant,
                strand,
                &turn,
                &content,
                Delivery::Explicit,
                &created,
            )?;
        }
        tx.execute("DELETE FROM early_replies WHERE turn_id = ?1", [&turn])
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub(super) fn insert_reply(
    tx: &Transaction<'_>,
    participant: &str,
    strand: &str,
    turn: &str,
    content: &str,
    delivery: Delivery,
    created: &str,
) -> Result<Message, String> {
    let id = format!("msg_{}", uuid::Uuid::new_v4().simple());
    tx.execute(
        r#"
        INSERT INTO messages (
          id, participant_id, author, strand_id, turn_id, delivery, content, created_at
        ) VALUES (?1, ?2, 'soul', ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            id,
            participant,
            strand,
            turn,
            delivery_db(delivery),
            content,
            created
        ],
    )
    .map_err(|error| error.to_string())?;
    message_by_turn(tx, turn)?.ok_or_else(|| "reply missing after insert".to_string())
}

fn delivery_db(value: Delivery) -> &'static str {
    match value {
        Delivery::Explicit => "explicit",
        Delivery::Automatic => "automatic",
    }
}

pub(super) fn now() -> String {
    Utc::now().to_rfc3339()
}
