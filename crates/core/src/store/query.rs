use rusqlite::{Connection, OptionalExtension};

use crate::{Author, Delivery, Message};

const COLUMNS: &str = "seq, id, participant_id, author, soul_id, strand_id, turn_id,
                   request_id, receipt_id, delivery, content, created_at";

pub(super) trait Query {
    fn held(&self) -> &Connection;

    fn requested(&self, request: &str) -> Result<Option<Message>, String> {
        self.found("request_id", request)
    }

    fn turned(&self, turn: &str) -> Result<Option<Message>, String> {
        self.found("turn_id", turn)
    }

    fn early(&self, turn: &str) -> Result<Option<(String, String)>, String> {
        self.held()
            .query_row(
                "SELECT strand_id, content FROM early_replies WHERE turn_id = ?1",
                [turn],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    fn pending(&self) -> Result<Vec<Message>, String> {
        let mut stmt = self
            .held()
            .prepare(&format!(
                r#"
                SELECT {COLUMNS}
                FROM messages
                WHERE author = 'human' AND receipt_id IS NULL AND request_id IS NOT NULL
                ORDER BY seq ASC
                "#
            ))
            .map_err(|error| error.to_string())?;
        let rows = stmt
            .query_map([], mapped)
            .map_err(|error| error.to_string())?;
        rows.map(|row| row.map_err(|error| error.to_string()))
            .collect()
    }

    fn found(&self, column: &str, value: &str) -> Result<Option<Message>, String> {
        self.held()
            .query_row(
                &format!("SELECT {COLUMNS} FROM messages WHERE {column} = ?1"),
                [value],
                mapped,
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    fn participant(&self, strand: &str) -> Result<Option<String>, String> {
        self.held()
            .query_row(
                "SELECT participant_id FROM conversations WHERE strand_id = ?1",
                [strand],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    fn cursor(&self) -> Result<i64, String> {
        self.held()
            .query_row(
                "SELECT value FROM state WHERE key = 'santi_cursor'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())
    }
}

impl Query for Connection {
    fn held(&self) -> &Connection {
        self
    }
}

pub(super) fn mapped(row: &rusqlite::Row<'_>) -> rusqlite::Result<Message> {
    Ok(Message {
        seq: row.get(0)?,
        id: row.get(1)?,
        participant: row.get(2)?,
        author: author(&row.get::<_, String>(3)?),
        soul: row.get(4)?,
        strand: row.get(5)?,
        turn: row.get(6)?,
        request: row.get(7)?,
        receipt: row.get(8)?,
        delivery: row
            .get::<_, Option<String>>(9)?
            .map(|value| delivery(&value)),
        content: row.get(10)?,
        created: row.get(11)?,
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
