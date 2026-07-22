use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
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
    let token = config.santi_token()?;
    let santi = crate::santi::Client::new(&config.santi.base_url, &token)?;
    let main = main_router(config.clone(), store.clone(), santi.clone());
    let replies = reply_router(config.clone(), store.clone(), santi.clone());
    workers(store, santi);
    let main_listener = tokio::net::TcpListener::bind(&config.listen.address)
        .await
        .with_context(|| format!("bind {}", config.listen.address))?;
    let reply_listener = tokio::net::TcpListener::bind(&config.reply.address)
        .await
        .with_context(|| format!("bind {}", config.reply.address))?;
    tokio::select! {
        result = axum::serve(main_listener, main) => result.context("serve stim"),
        result = axum::serve(reply_listener, replies) => result.context("serve stim replies"),
        _ = shutdown() => Ok(()),
    }
}

fn workers(store: stim_core::Store, santi: crate::santi::Client) {
    let event_store = store.clone();
    let event_client = santi.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = event_client.watch(event_store.clone()).await {
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

pub fn main_router(config: Config, store: stim_core::Store, santi: crate::santi::Client) -> Router {
    let app = state(config, store, santi);
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/messages", post(send))
        .route("/api/v1/inbox/{participant}", get(poll))
        .with_state(app)
}

pub fn reply_router(
    config: Config,
    store: stim_core::Store,
    santi: crate::santi::Client,
) -> Router {
    Router::new()
        .route("/api/v1/replies", post(reply))
        .with_state(state(config, store, santi))
}

fn state(config: Config, store: stim_core::Store, santi: crate::santi::Client) -> Arc<App> {
    Arc::new(App {
        config,
        store,
        santi,
    })
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true, "service": "stim" }))
}

async fn send(
    State(app): State<Arc<App>>,
    Json(mut request): Json<stim_core::MessageRequest>,
) -> Result<Json<stim_core::MessageResponse>, ApiError> {
    let soul = request
        .soul_id
        .take()
        .unwrap_or_else(|| app.config.santi.soul_id.clone());
    let staged = app
        .store
        .stage(
            &request.participant_id,
            &soul,
            &request.request_id,
            &request.content,
        )
        .map_err(ApiError::conflict)?;
    let receipt = app.santi.ingest(&staged.request).await.map_err(|error| {
        ApiError::upstream(anyhow::anyhow!(
            "{error}; request {} remains staged for retry",
            request.request_id
        ))
    })?;
    let message = app
        .store
        .accept(&request.request_id, &receipt)
        .map_err(ApiError::conflict)?;
    Ok(Json(stim_core::MessageResponse { message, receipt }))
}

async fn poll(
    State(app): State<Arc<App>>,
    Path(participant): Path<String>,
    Query(query): Query<Poll>,
) -> Result<Json<Vec<stim_core::Message>>, ApiError> {
    app.store
        .poll(&participant, query.since.unwrap_or(0))
        .map(Json)
        .map_err(ApiError::internal)
}

async fn reply(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Json(request): Json<stim_core::ReplyRequest>,
) -> Result<Json<stim_core::ReplyResponse>, ApiError> {
    let digest = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|token| hex::encode(Sha256::digest(token.as_bytes())))
        .unwrap_or_default();
    if !same(&digest, &app.config.reply.credential_sha256) {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid credential".to_string(),
        ));
    }
    app.store
        .reply(&request)
        .map(Json)
        .map_err(ApiError::conflict)
}

struct ApiError(StatusCode, String);

impl ApiError {
    fn conflict(error: String) -> Self {
        Self(StatusCode::CONFLICT, error)
    }

    fn internal(error: String) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, error)
    }

    fn upstream(error: anyhow::Error) -> Self {
        Self(StatusCode::BAD_GATEWAY, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
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
