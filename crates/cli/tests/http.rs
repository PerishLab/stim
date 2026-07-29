use std::sync::{Arc, Mutex};

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use plumb::config::{Kind, Listen, Store as StoreConfig};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use stim::config::{Config, Reply, Santi};
use stim_core::{Ingest, Store};
use tower::ServiceExt as _;

#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<Ingest>>>);

#[tokio::test]
async fn roundtrip() {
    let seen = Seen::default();
    let upstream = Router::new()
        .route("/api/v1/ingest", post(ingest))
        .route("/api/v1/turn-events", get(events))
        .route("/api/v1/turn-events/stream", get(stream))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind upstream");
    let address = listener.local_addr().expect("upstream address");
    tokio::spawn(async move {
        axum::serve(listener, upstream)
            .await
            .expect("serve upstream")
    });

    let temp = tempfile::tempdir().expect("temp dir");
    let store = Store::open(temp.path().join("stim.sqlite")).expect("store");
    let config = config(address.to_string(), temp.path().join("stim.sqlite"));
    let santi = stim::santi::Client::new(&format!("http://{address}"), "santi-secret")
        .expect("santi client");
    let app = stim::server::front(config.clone(), store.clone(), santi.clone());
    let replies = stim::server::back(config, store.clone(), santi.clone());

    let response = app
        .clone()
        .oneshot(framed(
            "/api/v1/messages",
            json!({
                "participant": "operator",
                "soul": null,
                "content": "hello",
                "request": "request_1"
            }),
            None,
        ))
        .await
        .expect("send response");
    assert_eq!(response.status(), StatusCode::OK);
    {
        let received = seen.0.lock().unwrap();
        assert_eq!(received.len(), 1);
        assert_eq!(received[0].label, "stim:operator");
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/inbox/operator?since=0")
                .body(Body::empty())
                .expect("poll request"),
        )
        .await
        .expect("poll response");
    assert_eq!(response.status(), StatusCode::OK);
    let messages = body(response).await;
    assert_eq!(messages.as_array().expect("messages").len(), 1);

    store
        .stage("operator", "soul_default", "request_2", "retry me")
        .expect("stage retry");
    assert_eq!(store.pending().expect("pending").len(), 1);
    assert_eq!(santi.recover(&store).await.expect("recover"), 1);
    assert!(store.pending().expect("recovered").is_empty());
    assert_eq!(seen.0.lock().unwrap().len(), 2);

    let reply = json!({
        "strand": "strand_1",
        "turn": "turn_1",
        "content": "early"
    });
    let missing = app
        .clone()
        .oneshot(framed("/api/v1/replies", reply.clone(), None))
        .await
        .expect("missing response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    let denied = replies
        .clone()
        .oneshot(framed("/api/v1/replies", reply.clone(), None))
        .await
        .expect("denied response");
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let accepted = replies
        .clone()
        .oneshot(framed(
            "/api/v1/replies",
            reply.clone(),
            Some("reply-secret"),
        ))
        .await
        .expect("reply response");
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted = body(accepted).await;
    assert_eq!(accepted["pending"], false);
    assert!(accepted["message"].is_object());
    let repeated = replies
        .oneshot(framed("/api/v1/replies", reply, Some("reply-secret")))
        .await
        .expect("repeat response");
    assert!(body(repeated).await["deduplicated"].as_bool().unwrap());

    let synced = santi.drain(&store).await.expect("backfill");
    assert_eq!(synced.cursor, 5);
    assert_eq!(synced.inserted, 0);
}

async fn ingest(
    State(seen): State<Seen>,
    headers: axum::http::HeaderMap,
    Json(request): Json<Ingest>,
) -> (StatusCode, Json<serde_json::Value>) {
    assert_eq!(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer santi-secret")
    );
    seen.0.lock().unwrap().push(request);
    (
        StatusCode::ACCEPTED,
        Json(json!({
            "strand": "strand_1",
            "inbox": "inbox_1",
            "warning": null
        })),
    )
}

async fn events() -> Json<serde_json::Value> {
    Json(json!({ "cursor": 5, "events": [] }))
}

async fn stream() -> &'static str {
    "event: turn_event_available\ndata: {}\n\n"
}

fn config(base: String, path: std::path::PathBuf) -> Config {
    Config {
        listen: Listen {
            host: "127.0.0.1".to_string(),
            port: 0,
            prefix: String::new(),
        },
        store: StoreConfig {
            kind: Kind::File,
            path: path.to_string_lossy().into_owned(),
        },
        santi: Santi {
            url: format!("http://{base}"),
            credential: "STIM_SANTI_TOKEN".to_string(),
            soul: "soul_default".to_string(),
        },
        reply: Reply {
            address: "127.0.0.1:0".to_string(),
            digest: hex::encode(Sha256::digest(b"reply-secret")),
        },
    }
}

fn framed(uri: &str, body: serde_json::Value, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder
        .body(Body::from(body.to_string()))
        .expect("json request")
}

async fn body(response: axum::response::Response) -> serde_json::Value {
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json body")
}
