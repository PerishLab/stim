use std::path::Path;

use anyhow::{Context, Result};
use plumb::config::{Cascade, Kind, Listen, Store};

#[derive(Clone, Debug, Cascade)]
pub struct Config {
    #[cascade(section)]
    pub listen: Listen,
    #[cascade(section)]
    pub store: Store,
    #[cascade(section)]
    pub santi: Santi,
    #[cascade(section)]
    pub reply: Reply,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: Listen {
                host: "127.0.0.1".to_string(),
                port: 43308,
                prefix: String::new(),
            },
            store: Store::default(),
            santi: Santi::default(),
            reply: Reply::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Cascade)]
#[cascade(section)]
pub struct Santi {
    pub url: String,
    pub credential: String,
    pub soul: String,
}

#[derive(Clone, Debug, Default, Cascade)]
#[cascade(section)]
pub struct Reply {
    pub address: String,
    pub digest: String,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut config = Self::resolve(Some(path))
            .with_context(|| format!("resolve config {}", path.display()))?;
        if config.store.kind != Kind::File {
            anyhow::bail!("store.kind must be file");
        }
        if config.reply.address.trim().is_empty() {
            anyhow::bail!("reply.address must not be empty");
        }
        if config.santi.url.trim().is_empty()
            || config.santi.credential.trim().is_empty()
            || config.santi.soul.trim().is_empty()
        {
            anyhow::bail!("santi url, credential, and soul are required");
        }
        let digest = config.reply.digest.trim().to_ascii_lowercase();
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            anyhow::bail!("reply.digest must be 64 hexadecimal characters");
        }
        config.reply.digest = digest;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        config.store.path = config.store.rebased(parent).to_string_lossy().into_owned();
        Ok(config)
    }

    pub fn token(&self) -> Result<String> {
        std::env::var(&self.santi.credential)
            .with_context(|| format!("missing credential env {}", self.santi.credential))
            .and_then(|value| {
                if value.trim().is_empty() {
                    anyhow::bail!("credential env {} is empty", self.santi.credential)
                }
                Ok(value)
            })
    }
}
