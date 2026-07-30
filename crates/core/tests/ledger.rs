use stim_core::{Delivery, Event, Events, Receipt, Reply, Store};

fn store() -> (tempfile::TempDir, Store) {
    let temp = tempfile::tempdir().expect("temp dir");
    let store = Store::open(temp.path().join("stim.sqlite")).expect("open store");
    (temp, store)
}

#[test]
fn durable() {
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
            &Receipt {
                strand: "strand_1".to_string(),
                inbox: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    assert_eq!(accepted.strand.as_deref(), Some("strand_1"));
    assert!(
        store
            .stage("other", "soul_default", "request_1", "hello")
            .is_err()
    );
}

#[test]
fn cursor() {
    let (_temp, store) = store();
    let event = event("turn_1", "complete");
    let first = store
        .sync(&Events {
            cursor: 7,
            events: vec![event.clone()],
        })
        .expect("sync");
    assert_eq!(first.inserted, 1);
    assert_eq!(store.cursor().expect("cursor"), 7);
    let second = store
        .sync(&Events {
            cursor: 9,
            events: vec![event],
        })
        .expect("repeat");
    assert_eq!(second.inserted, 0);
    assert_eq!(store.poll("operator", 0).expect("poll").len(), 1);
}

#[test]
fn explicit() {
    let (_temp, store) = store();
    store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("stage");
    store
        .accept(
            "request_1",
            &Receipt {
                strand: "strand_1".to_string(),
                inbox: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    let request = Reply {
        soul: "soul_default".to_string(),
        strand: "strand_1".to_string(),
        turn: "turn_1".to_string(),
        call: "call_1".to_string(),
        effect: "effect_1".to_string(),
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
        .sync(&Events {
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
fn early() {
    let (_temp, store) = store();
    store
        .stage("operator", "soul_default", "request_1", "hello")
        .expect("stage");
    let request = Reply {
        soul: "soul_default".to_string(),
        strand: "strand_1".to_string(),
        turn: "turn_1".to_string(),
        call: "call_1".to_string(),
        effect: "effect_1".to_string(),
        content: "early".to_string(),
    };
    let pending = store.reply(&request).expect("pending reply");
    assert!(pending.pending);
    assert!(pending.message.is_none());
    assert!(store.reply(&request).expect("repeat pending").deduplicated);
    store
        .accept(
            "request_1",
            &Receipt {
                strand: "strand_1".to_string(),
                inbox: "inbox_1".to_string(),
                warning: None,
            },
        )
        .expect("accept");
    let messages = store.poll("operator", 0).expect("poll");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1].content, "early");
    assert_eq!(messages[1].delivery, Some(Delivery::Explicit));
}

fn event(turn: &str, text: &str) -> Event {
    Event {
        id: format!("event_{turn}"),
        strand: "strand_1".to_string(),
        turn: turn.to_string(),
        label: "stim:operator".to_string(),
        text: text.to_string(),
        completed: "2026-07-22T00:00:00Z".to_string(),
    }
}
