use std::path::Path;

use anyhow::{Context, Result};
use plumb::config::{Kind, Listen, Store};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct Config {
    pub listen: Listen,
    pub store: Store,
    pub santi: Santi,
    pub reply: Reply,
}

#[derive(Clone, Deserialize)]
pub struct Santi {
    pub base_url: String,
    pub credential_env: String,
    pub soul_id: String,
}

#[derive(Clone, Deserialize)]
pub struct Reply {
    pub address: String,
    pub credential_sha256: String,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read config {}", path.display()))?;
        let mut config: Self = toml::from_str(&text).context("parse stim config")?;
        if config.store.kind != Kind::File {
            anyhow::bail!("store.kind must be file");
        }
        if config.reply.address.trim().is_empty() {
            anyhow::bail!("reply.address must not be empty");
        }
        if config.santi.base_url.trim().is_empty()
            || config.santi.credential_env.trim().is_empty()
            || config.santi.soul_id.trim().is_empty()
        {
            anyhow::bail!("santi base_url, credential_env, and soul_id are required");
        }
        let digest = config.reply.credential_sha256.trim().to_ascii_lowercase();
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!("reply.credential_sha256 must be 64 hexadecimal characters");
        }
        config.reply.credential_sha256 = digest;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        config.store.path = config.store.rebased(parent).to_string_lossy().into_owned();
        Ok(config)
    }

    pub fn santi_token(&self) -> Result<String> {
        std::env::var(&self.santi.credential_env)
            .with_context(|| format!("missing credential env {}", self.santi.credential_env))
            .and_then(|value| {
                if value.trim().is_empty() {
                    anyhow::bail!("credential env {} is empty", self.santi.credential_env)
                }
                Ok(value)
            })
    }
}
