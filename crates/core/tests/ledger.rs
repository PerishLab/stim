use stim_core::{Delivery, IngestReceipt, ReplyRequest, Store, TurnEvent, TurnEventBatch};

fn store() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().expect("temp dir");
    let store = Store::open(temp.path().join("stim.sqlite")).expect("open store");
    (temp, store)
}

#[test]
fn send_is_durable_and_idempotent() {
    let (_temp, store) = store();
    let staged = store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("stage");
    assert_eq!(staged.request.label, "stim:operator");
    let repeated = store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("repeat");
    assert_eq!(repeated.message.id, staged.message.id);
    let accepted = store
        .accept(
            "request_1",
            &IngestReceipt {
                strand_id: "strand_1".to_string(),
                inbox_id: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    assert_eq!(accepted.strand_id.as_deref(), Some("strand_1"));
    assert!(
        store
            .stage("other", "soul_default", "request_1", "hello")
            .is_err()
    );
}

#[test]
fn cursor_advances_and_automatic_replies_deduplicate() {
    let (_temp, store) = store();
    let event = event("turn_1", "complete");
    let first = store
        .sync(&TurnEventBatch {
            cursor: 7,
            events: vec![event.clone()],
        })
        .expect("sync");
    assert_eq!(first.inserted, 1);
    assert_eq!(store.cursor().expect("cursor"), 7);
    let second = store
        .sync(&TurnEventBatch {
            cursor: 9,
            events: vec![event],
        })
        .expect("repeat");
    assert_eq!(second.inserted, 0);
    assert_eq!(store.poll("operator", 0).expect("poll").len(), 1);
}

#[test]
fn explicit_reply_wins_over_completion() {
    let (_temp, store) = store();
    store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("stage");
    store
        .accept(
            "request_1",
            &IngestReceipt {
                strand_id: "strand_1".to_string(),
                inbox_id: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    let request = ReplyRequest {
        strand_id: "strand_1".to_string(),
        turn_id: "turn_1".to_string(),
        content: "early".to_string(),
    };
    let first = store.reply(&request).expect("reply");
    assert!(!first.deduplicated);
    assert_eq!(
        first.message.as_ref().and_then(|message| message.delivery),
        Some(Delivery::Explicit)
    );
    assert!(store.reply(&request).expect("repeat").deduplicated);
    let synced = store
        .sync(&TurnEventBatch {
            cursor: 3,
            events: vec![event("turn_1", "final")],
        })
        .expect("sync final");
    assert_eq!(synced.inserted, 0);
    let messages = store.poll("operator", 0).expect("poll");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "early");
}

#[test]
fn early_reply_waits_for_an_ambiguous_ingest_mapping() {
    let (_temp, store) = store();
    store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("stage");
    let request = ReplyRequest {
        strand_id: "strand_1".to_string(),
        turn_id: "turn_1".to_string(),
        content: "early".to_string(),
    };
    let pending = store.reply(&request).expect("pending reply");
    assert!(pending.pending);
    assert!(pending.message.is_none());
    assert!(store.reply(&request).expect("repeat pending").deduplicated);
    store
        .accept(
            "request_1",
            &IngestReceipt {
                strand_id: "strand_1".to_string(),
                inbox_id: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    let messages = store.poll("operator", 0).expect("poll");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "early");
    assert_eq!(messages[1].delivery, Some(Delivery::Explicit));
}

fn event(turn: &str, text: &str) -> TurnEvent {
    TurnEvent {
        id: format!("event_{turn}"),
        strand_id: "strand_1".to_string(),
        turn_id: turn.to_string(),
        external_label: "stim:operator".to_string(),
        final_text: text.to_string(),
        completed_at: "2026-07-22T00:00:00Z".to_string(),
    }
}
