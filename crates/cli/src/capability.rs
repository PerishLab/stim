use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;

const PREFIX: &str = "santi1";
const SCHEMA: &str = "santi.runtime-capability.v1";
const SKEW: u64 = 30;
const LIMIT: usize = 8192;

#[derive(Clone)]
pub(crate) struct Verifier {
    issuer: String,
    audience: String,
    lifetime: u64,
    keys: BTreeMap<String, VerifyingKey>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    schema: String,
    iss: String,
    aud: String,
    kid: String,
    soul: String,
    strand: String,
    turn: String,
    call: String,
    effect: String,
    iat: u64,
    exp: u64,
}

#[derive(PartialEq, Eq)]
struct Origin<'a> {
    soul: &'a str,
    strand: &'a str,
    turn: &'a str,
    call: &'a str,
    effect: &'a str,
}

#[derive(PartialEq, Eq)]
struct Envelope {
    prefix: bool,
    complete: bool,
    payload: bool,
    signature: bool,
}

impl Verifier {
    pub(crate) fn new(
        issuer: &str,
        audience: &str,
        lifetime: u64,
        encoded: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        let issuer = required("reply.issuer", issuer, 256)?;
        let audience = required("reply.audience", audience, 256)?;
        if lifetime == 0 {
            return Err("reply.maximum_ttl_seconds must be greater than zero".to_string());
        }
        if encoded.is_empty() {
            return Err("reply_keys must contain at least one public key".to_string());
        }
        let mut keys = BTreeMap::new();
        for (kid, encoded) in encoded {
            let kid = required("reply key id", kid, 128)?;
            let bytes = URL_SAFE_NO_PAD
                .decode(encoded.trim())
                .map_err(|_| format!("reply key {kid} is not unpadded base64url"))?;
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| format!("reply key {kid} must decode to 32 bytes"))?;
            let key = VerifyingKey::from_bytes(&bytes)
                .map_err(|_| format!("reply key {kid} is not an Ed25519 public key"))?;
            if keys.insert(kid, key).is_some() {
                return Err("reply key ids must be unique".to_string());
            }
        }
        Ok(Self {
            issuer,
            audience,
            lifetime,
            keys,
        })
    }

    pub(crate) fn verify(
        &self,
        token: &str,
        request: &stim_core::Reply,
    ) -> Result<(), &'static str> {
        self.inspect(token, request, epoch()?)
    }

    fn inspect(
        &self,
        token: &str,
        request: &stim_core::Reply,
        now: u64,
    ) -> Result<(), &'static str> {
        if token.len() > LIMIT {
            return Err("capability is too long");
        }
        let mut parts = token.split('.');
        let prefix = parts.next().ok_or("capability prefix is missing")?;
        let payload = parts.next().ok_or("capability payload is missing")?;
        let signature = parts.next().ok_or("capability signature is missing")?;
        let envelope = Envelope {
            prefix: prefix == PREFIX,
            complete: parts.next().is_none(),
            payload: !payload.is_empty(),
            signature: !signature.is_empty(),
        };
        let expected = Envelope {
            prefix: true,
            complete: true,
            payload: true,
            signature: true,
        };
        if envelope != expected {
            return Err("capability envelope is malformed");
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| "capability payload is not unpadded base64url")?;
        let claims: Claims =
            serde_json::from_slice(&decoded).map_err(|_| "capability claims are malformed")?;
        let key = self
            .keys
            .get(&claims.kid)
            .ok_or("capability key id is not trusted")?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| "capability signature is not unpadded base64url")?;
        let signature =
            Signature::from_slice(&signature).map_err(|_| "capability signature is malformed")?;
        let signed = format!("{PREFIX}.{payload}");
        key.verify_strict(signed.as_bytes(), &signature)
            .map_err(|_| "capability signature does not verify")?;

        if claims.schema != SCHEMA {
            return Err("capability schema is not supported");
        }
        bounded(&claims.iss, 256)?;
        bounded(&claims.aud, 256)?;
        bounded(&claims.kid, 128)?;
        bounded(&claims.soul, 256)?;
        bounded(&claims.strand, 256)?;
        bounded(&claims.turn, 256)?;
        bounded(&claims.call, 256)?;
        bounded(&claims.effect, 256)?;
        if claims.iss != self.issuer || claims.aud != self.audience {
            return Err("capability authority does not match");
        }
        if claims.exp <= claims.iat
            || claims.exp - claims.iat > self.lifetime
            || claims.iat > now.saturating_add(SKEW)
        {
            return Err("capability lifetime is invalid");
        }
        if now >= claims.exp {
            return Err("capability has expired");
        }
        let origin = Origin {
            soul: &claims.soul,
            strand: &claims.strand,
            turn: &claims.turn,
            call: &claims.call,
            effect: &claims.effect,
        };
        let expected = Origin {
            soul: &request.soul,
            strand: &request.strand,
            turn: &request.turn,
            call: &request.call,
            effect: &request.effect,
        };
        if origin != expected {
            return Err("capability origin does not match the request");
        }
        Ok(())
    }
}

fn required(name: &str, value: &str, limit: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > limit {
        return Err(format!("{name} must not exceed {limit} bytes"));
    }
    Ok(value.to_string())
}

fn bounded(value: &str, limit: usize) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > limit {
        return Err("capability claim is outside its bounds");
    }
    Ok(())
}

fn epoch() -> Result<u64, &'static str> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is before the Unix epoch")
}
