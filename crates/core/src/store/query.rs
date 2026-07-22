use rusqlite::{Connection, OptionalExtension};

use crate::{Author, Delivery, Message};

pub(super) fn message_by_request(
    conn: &Connection,
    request: &str,
) -> Result<Option<Message>, String> {
    query_message(conn, "request_id", request)
}

pub(super) fn message_by_turn(conn: &Connection, turn: &str) -> Result<Option<Message>, String> {
    query_message(conn, "turn_id", turn)
}

pub(super) fn early_by_turn(
    conn: &Connection,
    turn: &str,
) -> Result<Option<(String, String)>, String> {
    conn.query_row(
        "SELECT strand_id, content FROM early_replies WHERE turn_id = ?1",
        [turn],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(super) fn pending(conn: &Connection) -> Result<Vec<Message>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT seq, id, participant_id, author, soul_id, strand_id, turn_id,
                   request_id, receipt_id, delivery, content, created_at
            FROM messages
            WHERE author = 'human' AND receipt_id IS NULL AND request_id IS NOT NULL
            ORDER BY seq ASC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map([], map_message)
        .map_err(|error| error.to_string())?;
    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

fn query_message(conn: &Connection, column: &str, value: &str) -> Result<Option<Message>, String> {
    conn.query_row(
        &format!(
            r#"
            SELECT seq, id, participant_id, author, soul_id, strand_id, turn_id,
                   request_id, receipt_id, delivery, content, created_at
            FROM messages WHERE {column} = ?1
            "#
        ),
        [value],
        map_message,
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(super) fn participant_for_strand(
    conn: &Connection,
    strand: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT participant_id FROM conversations WHERE strand_id = ?1",
        [strand],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(super) fn cursor(conn: &Connection) -> Result<i64, String> {
    conn.query_row(
        "SELECT value FROM state WHERE key = 'santi_cursor'",
        [],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

pub(super) fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        seq: row.get(0)?,
        id: row.get(1)?,
        participant_id: row.get(2)?,
        author: author(&row.get::<_, String>(3)?),
        soul_id: row.get(4)?,
        strand_id: row.get(5)?,
        turn_id: row.get(6)?,
        request_id: row.get(7)?,
        receipt_id: row.get(8)?,
        delivery: row
            .get::<_, Option<String>>(9)?
            .map(|value| delivery(&value)),
        content: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn author(value: &str) -> Author {
    if value == "human" {
        Author::Human
    } else {
        Author::Soul
    }
}

fn delivery(value: &str) -> Delivery {
    if value == "explicit" {
        Delivery::Explicit
    } else {
        Delivery::Automatic
    }
}
