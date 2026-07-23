use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    base: String,
}

impl Client {
    pub fn new(base: &str, token: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut auth = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .context("invalid santi bearer")?;
        auth.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, auth);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .context("build santi client")?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
        })
    }

    pub async fn ingest(&self, request: &stim_core::Ingest) -> Result<stim_core::Receipt> {
        let response = self
            .http
            .post(format!("{}/api/v1/ingest", self.base))
            .timeout(Duration::from_secs(30))
            .json(request)
            .send()
            .await
            .context("post santi ingest")?;
        decode(response, "santi ingest").await
    }

    pub async fn backfill(&self, since: i64) -> Result<stim_core::Events> {
        let response = self
            .http
            .get(format!("{}/api/v1/turn-events", self.base))
            .query(&[("since", since), ("limit", 256)])
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context("get santi turn events")?;
        decode(response, "santi turn events").await
    }

    pub async fn watch(&self, store: stim_core::Store) -> Result<()> {
        loop {
            self.drain(&store).await?;
            let response = self
                .http
                .get(format!("{}/api/v1/turn-events/stream", self.base))
                .send()
                .await
                .context("open santi turn event stream")?;
            if !response.status().is_success() {
                anyhow::bail!("santi turn event stream returned {}", response.status());
            }
            let mut bytes = response.bytes_stream();
            let mut buffer = String::new();
            while let Some(chunk) = bytes.next().await {
                let chunk = chunk.context("read santi turn event stream")?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));
                while let Some((index, delimiter)) = boundary(&buffer) {
                    let frame = buffer[..index].to_string();
                    buffer.drain(..index + delimiter);
                    refresh(self, &store, &frame).await?;
                }
            }
        }
    }

    pub async fn drain(&self, store: &stim_core::Store) -> Result<stim_core::Synced> {
        let mut total = 0;
        loop {
            let before = store.cursor().map_err(anyhow::Error::msg)?;
            let batch = self.backfill(before).await?;
            let count = batch.events.len();
            let synced = store.sync(&batch).map_err(anyhow::Error::msg)?;
            total += synced.inserted;
            if batch.cursor == before || count < 256 {
                return Ok(stim_core::Synced {
                    cursor: synced.cursor,
                    inserted: total,
                });
            }
        }
    }

    pub async fn recover(&self, store: &stim_core::Store) -> Result<usize> {
        let pending = store.pending().map_err(anyhow::Error::msg)?;
        for staged in &pending {
            let receipt = self.ingest(&staged.request).await?;
            store
                .accept(&staged.request.request, &receipt)
                .map_err(anyhow::Error::msg)?;
        }
        Ok(pending.len())
    }
}

pub fn boundary(buffer: &str) -> Option<(usize, usize)> {
    match (buffer.find("\n\n"), buffer.find("\r\n\r\n")) {
        (Some(left), Some(right)) if left <= right => Some((left, 2)),
        (Some(_), Some(right)) => Some((right, 4)),
        (Some(index), None) => Some((index, 2)),
        (None, Some(index)) => Some((index, 4)),
        (None, None) => None,
    }
}

fn wake(frame: &str) -> bool {
    frame.lines().any(|line| line.starts_with("data:"))
}

async fn refresh(client: &Client, store: &stim_core::Store, frame: &str) -> Result<()> {
    if wake(frame) {
        client.drain(store).await?;
    }
    Ok(())
}

async fn decode<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
    operation: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.text().await.context("read santi response")?;
    if !status.is_success() {
        anyhow::bail!("{operation} failed with {status}: {body}");
    }
    serde_json::from_str(&body).with_context(|| format!("decode {operation} response"))
}
