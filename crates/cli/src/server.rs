use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::config::Config;

#[derive(Clone)]
struct App {
    config: Config,
    store: stim_core::Store,
    santi: crate::santi::Client,
}

#[derive(Deserialize)]
struct Poll {
    since: Option<i64>,
}

pub async fn serve(config: Config) -> Result<()> {
    let store = stim_core::Store::open(&config.store.path).map_err(anyhow::Error::msg)?;
    let token = config.token()?;
    let santi = crate::santi::Client::new(&config.santi.url, &token)?;
    let main = front(config.clone(), store.clone(), santi.clone());
    let replies = back(config.clone(), store.clone(), santi.clone());
    workers(store, santi);
    let door = tokio::net::TcpListener::bind(config.listen.address())
        .await
        .with_context(|| format!("bind {}", config.listen.address()))?;
    let gate = tokio::net::TcpListener::bind(&config.reply.address)
        .await
        .with_context(|| format!("bind {}", config.reply.address))?;
    tokio::select! {
        result = axum::serve(door, main) => result.context("serve stim"),
        result = axum::serve(gate, replies) => result.context("serve stim replies"),
        _ = shutdown() => Ok(()),
    }
}

fn workers(store: stim_core::Store, santi: crate::santi::Client) {
    let held = store.clone();
    let client = santi.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = client.watch(held.clone()).await {
                eprintln!("stim: santi consumer reconnecting: {error}");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    });
    tokio::spawn(async move {
        loop {
            if let Err(error) = santi.recover(&store).await {
                eprintln!("stim: staged ingest remains pending: {error}");
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}

pub fn front(config: Config, store: stim_core::Store, santi: crate::santi::Client) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/messages", post(send))
        .route("/api/v1/inbox/{participant}", get(poll))
        .with_state(Arc::new(App {
            config,
            store,
            santi,
        }))
}

pub fn back(config: Config, store: stim_core::Store, santi: crate::santi::Client) -> Router {
    Router::new()
        .route("/api/v1/replies", post(reply))
        .with_state(Arc::new(App {
            config,
            store,
            santi,
        }))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "service": "stim" }))
}

async fn send(
    State(app): State<Arc<App>>,
    Json(mut request): Json<stim_core::Post>,
) -> Result<Json<stim_core::Posted>, Fault> {
    let soul = request
        .soul
        .take()
        .unwrap_or_else(|| app.config.santi.soul.clone());
    let staged = app
        .store
        .stage(
            &request.participant,
            &soul,
            &request.request,
            &request.content,
        )
        .map_err(conflict)?;
    let receipt = app.santi.ingest(&staged.request).await.map_err(|error| {
        upstream(anyhow::anyhow!(
            "{error}; request {} remains staged for retry",
            request.request
        ))
    })?;
    let message = app
        .store
        .accept(&request.request, &receipt)
        .map_err(conflict)?;
    Ok(Json(stim_core::Posted { message, receipt }))
}

async fn poll(
    State(app): State<Arc<App>>,
    Path(participant): Path<String>,
    Query(query): Query<Poll>,
) -> Result<Json<Vec<stim_core::Message>>, Fault> {
    app.store
        .poll(&participant, query.since.unwrap_or(0))
        .map(Json)
        .map_err(internal)
}

async fn reply(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(request): Json<stim_core::Reply>,
) -> Result<Json<stim_core::Replied>, Fault> {
    let digest = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| hex::encode(Sha256::digest(token.as_bytes())))
        .unwrap_or_default();
    if !same(&digest, &app.config.reply.digest) {
        return Err(fault(
            StatusCode::UNAUTHORIZED,
            "invalid credential".to_string(),
        ));
    }
    app.store.reply(&request).map(Json).map_err(conflict)
}

type Fault = (StatusCode, Json<serde_json::Value>);

fn fault(status: StatusCode, error: String) -> Fault {
    (status, Json(serde_json::json!({ "error": error })))
}

fn conflict(error: String) -> Fault {
    fault(StatusCode::CONFLICT, error)
}

fn internal(error: String) -> Fault {
    fault(StatusCode::INTERNAL_SERVER_ERROR, error)
}

fn upstream(error: anyhow::Error) -> Fault {
    fault(StatusCode::BAD_GATEWAY, error.to_string())
}

fn same(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
