use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use plumb::config::Cascade;
use serde::Deserialize;

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
    pub reply_keys: BTreeMap<String, String>,
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
            reply_keys: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Cascade, Deserialize, PartialEq, Eq)]
#[cascade(section)]
#[serde(default)]
pub struct Listen {
    pub host: String,
    pub port: u16,
    pub prefix: String,
}

impl Default for Listen {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 3000,
            prefix: String::new(),
        }
    }
}

impl Listen {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Memory,
    File,
}

impl plumb::config::Env for Kind {
    fn read(value: &str) -> std::result::Result<Self, String> {
        match value {
            "memory" => Ok(Self::Memory),
            "file" => Ok(Self::File),
            _ => Err("neither memory nor file".to_string()),
        }
    }
}

#[derive(Clone, Debug, Default, Cascade, Deserialize, PartialEq, Eq)]
#[cascade(section)]
#[serde(default)]
pub struct Store {
    pub kind: Kind,
    pub path: String,
}

impl Store {
    pub fn rebased(&self, base: &Path) -> std::path::PathBuf {
        plumb::config::rebase(Path::new(&self.path), base)
    }
}

#[derive(Clone, Debug, Default, Cascade)]
#[cascade(section)]
pub struct Santi {
    pub url: String,
    pub credential: String,
    pub soul: String,
}

#[derive(Clone, Debug, Cascade)]
#[cascade(section)]
pub struct Reply {
    pub address: String,
    pub issuer: String,
    pub audience: String,
    pub maximum_ttl_seconds: u64,
}

impl Default for Reply {
    fn default() -> Self {
        Self {
            address: String::new(),
            issuer: String::new(),
            audience: String::new(),
            maximum_ttl_seconds: 300,
        }
    }
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
        crate::capability::Verifier::new(
            &config.reply.issuer,
            &config.reply.audience,
            config.reply.maximum_ttl_seconds,
            &config.reply_keys,
        )
        .map_err(anyhow::Error::msg)?;
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
