pub(super) const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS conversations (
    participant_id TEXT PRIMARY KEY,
    soul_id TEXT,
    strand_id TEXT UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS messages (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    participant_id TEXT NOT NULL,
    author TEXT NOT NULL CHECK (author IN ('human', 'soul')),
    soul_id TEXT,
    strand_id TEXT,
    turn_id TEXT,
    request_id TEXT,
    receipt_id TEXT,
    delivery TEXT CHECK (delivery IS NULL OR delivery IN ('explicit', 'automatic')),
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (participant_id) REFERENCES conversations(participant_id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_request
ON messages(request_id) WHERE request_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_turn
ON messages(turn_id) WHERE turn_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_messages_participant_seq
ON messages(participant_id, seq);
CREATE TABLE IF NOT EXISTS early_replies (
    turn_id TEXT PRIMARY KEY,
    strand_id TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_early_replies_strand
ON early_replies(strand_id);
CREATE TABLE IF NOT EXISTS state (
    key TEXT PRIMARY KEY,
    value INTEGER NOT NULL
);
INSERT OR IGNORE INTO state (key, value) VALUES ('santi_cursor', 0);
PRAGMA user_version = 1;
"#;
