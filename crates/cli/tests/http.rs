use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer as _, SigningKey};
use serde_json::json;
use stim::config::{Config, Kind, Listen, Reply, Santi, Store as StoreConfig};
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
    let replies = stim::server::back(config, store.clone()).expect("reply app");

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
        "soul": "soul_default",
        "strand": "strand_1",
        "turn": "turn_1",
        "call": "call_1",
        "effect": "effect_1",
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
    let legacy = replies
        .clone()
        .oneshot(framed(
            "/api/v1/replies",
            reply.clone(),
            Some("legacy-static-token"),
        ))
        .await
        .expect("legacy response");
    assert_eq!(legacy.status(), StatusCode::UNAUTHORIZED);
    let capability = capability(&reply);
    let mut mismatched = reply.clone();
    mismatched["turn"] = json!("turn_other");
    let denied = replies
        .clone()
        .oneshot(framed("/api/v1/replies", mismatched, Some(&capability)))
        .await
        .expect("mismatch response");
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let accepted = replies
        .clone()
        .oneshot(framed("/api/v1/replies", reply.clone(), Some(&capability)))
        .await
        .expect("reply response");
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted = body(accepted).await;
    assert_eq!(accepted["pending"], false);
    assert!(accepted["message"].is_object());
    let mut changed = reply.clone();
    changed["content"] = json!("changed");
    let conflict = replies
        .clone()
        .oneshot(framed("/api/v1/replies", changed, Some(&capability)))
        .await
        .expect("conflict response");
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let repeated = replies
        .oneshot(framed("/api/v1/replies", reply, Some(&capability)))
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
            issuer: "santi.example".to_string(),
            audience: "stim.reply".to_string(),
            maximum_ttl_seconds: 300,
        },
        reply_keys: [(
            "test-2026".to_string(),
            URL_SAFE_NO_PAD.encode(SigningKey::from_bytes(&[7; 32]).verifying_key().as_bytes()),
        )]
        .into_iter()
        .collect(),
    }
}

fn capability(reply: &serde_json::Value) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs();
    let payload = json!({
        "schema": "santi.runtime-capability.v1",
        "iss": "santi.example",
        "aud": "stim.reply",
        "kid": "test-2026",
        "soul": reply["soul"],
        "strand": reply["strand"],
        "turn": reply["turn"],
        "call": reply["call"],
        "effect": reply["effect"],
        "iat": now,
        "exp": now + 120,
    });
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).expect("payload"));
    let signed = format!("santi1.{payload}");
    let signature = SigningKey::from_bytes(&[7; 32]).sign(signed.as_bytes());
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
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
