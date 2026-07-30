use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer as _, SigningKey};
use serde::Serialize;
use stim::config::{Config, Kind, Listen, Reply, Santi, Store as StoreConfig};
use stim_core::Store;
use tower::ServiceExt as _;

#[derive(Clone, Serialize)]
struct Claims {
    schema: &'static str,
    iss: &'static str,
    aud: &'static str,
    kid: &'static str,
    soul: String,
    strand: String,
    turn: String,
    call: String,
    effect: String,
    iat: u64,
    exp: u64,
}

#[tokio::test]
async fn boundaries() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(temp.path().join("stim.sqlite")).unwrap();
    let app = stim::server::back(config(temp.path()), store).unwrap();
    let reply = request();
    let signing = SigningKey::from_bytes(&[7; 32]);

    denied(&app, &reply, "legacy-static-token").await;
    let mut held = claims(&reply);
    held.exp = held.iat - 1;
    denied(&app, &reply, &token(&signing, &held)).await;
    let mut held = claims(&reply);
    held.iss = "another";
    denied(&app, &reply, &token(&signing, &held)).await;
    let mut held = claims(&reply);
    held.aud = "another";
    denied(&app, &reply, &token(&signing, &held)).await;
    let mut held = claims(&reply);
    held.kid = "unknown";
    denied(
        &app,
        &reply,
        &token(&SigningKey::from_bytes(&[9; 32]), &held),
    )
    .await;
    for wrong in origins() {
        denied(&app, &wrong, &token(&signing, &claims(&reply))).await;
    }
    let mut held = token(&signing, &claims(&reply)).into_bytes();
    let last = held.len() - 1;
    held[last] = if held[last] == b'A' { b'B' } else { b'A' };
    denied(&app, &reply, std::str::from_utf8(&held).unwrap()).await;

    let retiring = SigningKey::from_bytes(&[8; 32]);
    let mut held = claims(&reply);
    held.kid = "retiring";
    let response = app
        .oneshot(framed(&reply, &token(&retiring, &held)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn denied(app: &Router, reply: &stim_core::Reply, token: &str) {
    let response = app.clone().oneshot(framed(reply, token)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

fn framed(reply: &stim_core::Reply, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/api/v1/replies")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(serde_json::to_vec(reply).unwrap()))
        .unwrap()
}

fn claims(reply: &stim_core::Reply) -> Claims {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Claims {
        schema: "santi.runtime-capability.v1",
        iss: "santi.example",
        aud: "stim.reply",
        kid: "active",
        soul: reply.soul.clone(),
        strand: reply.strand.clone(),
        turn: reply.turn.clone(),
        call: reply.call.clone(),
        effect: reply.effect.clone(),
        iat: now,
        exp: now + 120,
    }
}

fn token(signing: &SigningKey, claims: &Claims) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(claims).unwrap());
    let signed = format!("santi1.{payload}");
    let signature = signing.sign(signed.as_bytes());
    format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()))
}

fn origins() -> Vec<stim_core::Reply> {
    ["soul", "strand", "turn", "call", "effect"]
        .into_iter()
        .map(|field| {
            let mut held = request();
            match field {
                "soul" => held.soul = "other".to_string(),
                "strand" => held.strand = "other".to_string(),
                "turn" => held.turn = "other".to_string(),
                "call" => held.call = "other".to_string(),
                _ => held.effect = "other".to_string(),
            }
            held
        })
        .collect()
}

fn request() -> stim_core::Reply {
    stim_core::Reply {
        soul: "soul_1".to_string(),
        strand: "strand_1".to_string(),
        turn: "turn_1".to_string(),
        call: "call_1".to_string(),
        effect: "effect_1".to_string(),
        content: "early".to_string(),
    }
}

fn config(path: &std::path::Path) -> Config {
    let active = SigningKey::from_bytes(&[7; 32]);
    let retiring = SigningKey::from_bytes(&[8; 32]);
    Config {
        listen: Listen {
            host: "127.0.0.1".to_string(),
            port: 0,
            prefix: String::new(),
        },
        store: StoreConfig {
            kind: Kind::File,
            path: path.join("stim.sqlite").to_string_lossy().into_owned(),
        },
        santi: Santi {
            url: "http://127.0.0.1:1".to_string(),
            credential: "STIM_SANTI_TOKEN".to_string(),
            soul: "soul_default".to_string(),
        },
        reply: Reply {
            address: "127.0.0.1:0".to_string(),
            issuer: "santi.example".to_string(),
            audience: "stim.reply".to_string(),
            maximum_ttl_seconds: 300,
        },
        reply_keys: [("active", &active), ("retiring", &retiring)]
            .into_iter()
            .map(|(kid, key)| {
                (
                    kid.to_string(),
                    URL_SAFE_NO_PAD.encode(key.verifying_key().as_bytes()),
                )
            })
            .collect(),
    }
}
