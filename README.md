# stim

`stim` is a standalone message product downstream of [santi](https://git.perish.top/PerishFire/santi).
It owns participants, the durable message ledger, delivery projection, and its HTTP/CLI
surface. Santi remains authoritative for souls, strands, turns, and provider execution.

The default topology is cross-host:

```text
operator -> stim main API -> Santi /ingest
                            Santi /turn-events + SSE -> stim ledger
santi shell -> stim reply-only API
```

Santi authorizes stim's opaque `stim:` label zone with one downstream bearer. The
reply-only listener instead trusts short-lived Ed25519 capabilities signed for one
concrete Santi shell effect. Turn events are consumed by cursor backfill; SSE is only a
wake-up, so reconnects do not lose replies.

## Configure

Copy `stim.example.toml` to the gitignored `stim.toml`, then set the Santi downstream
token in the configured environment variable:

```sh
cp stim.example.toml stim.toml
export STIM_SANTI_TOKEN='<raw token registered with santi>'
```

Register only the SHA-256 digest of that token with Santi's operator-authenticated
`POST /api/v1/downstreams` endpoint:

```json
{
  "id": "stim",
  "prefix": "stim:",
  "digest": "<64 lowercase hexadecimal characters>"
}
```

Configure the exact Santi issuer and audience under `[reply]`, then register its
public key by `kid`:

```toml
[reply]
address = "0.0.0.0:43309"
issuer = "santi.example.com"
audience = "stim.reply"
maximum_ttl_seconds = 300

[reply_keys]
key_2026 = "<unpadded base64url Ed25519 public key>"
```

`listen.address` serves the main user API and defaults to loopback. Put it behind the
user identity boundary before exposing it. `reply.address` is the cross-host listener;
it exposes only `POST /api/v1/replies` and verifies the capability signature, authority,
lifetime, and request-bound origin. Terminate TLS at the host edge; the service listeners
themselves speak HTTP.

## Run

```sh
cargo run --locked -p stim -- service serve
```

For local lifecycle, the tracked `sidecar.toml` starts the same service:

```sh
sidecar start --config sidecar.toml
```

## Use

```sh
stim send 'hello' --as operator
stim poll --as operator --since 0
```

An early reply runs on the Santi host during a turn. Santi already injects
the complete shell origin and a fresh `SANTI_RUNTIME_CAPABILITY`; only the remote
endpoint remains a soul or strand declaration:

```sh
santi env set soul soul_default STIM_BASE_URL https://stim.example.com:43309
stim reply 'I am still working'
```

The Stim CLI copies soul, strand, turn, tool-call, and effect ids from Santi's
reserved environment into the request and presents the capability. Stim requires
every value to equal the signed claim. The private signing key never enters the
shell or either estate, and detached Santi jobs receive no runtime capability.

For rotation, add the new `kid` and public key before switching Santi. Keep the
retiring key through `maximum_ttl_seconds`, then remove it.

Repeating the same reply for the same turn is idempotent. A different payload for an
already-used turn is rejected. If an explicit early reply exists, the automatic final
completion for that turn is acknowledged but does not replace it. A reply that arrives
before the ingest receipt is durably reported as `pending` and materializes once the
strand is bound.

## Develop

```sh
runseal :init
runseal :guard
```

The guard runs Rust formatting, clippy, tests, Deno checks, and the repository's
ectropy constitution. `plumb.toml` records the binary release shape; stable
Plumb and the shared Actions workflow own build, packaging, managers, and
sealed delivery. Publishing remains inert until Stim enters that lifecycle and
its release capabilities are provisioned.

Stable is the canonical default install:

```sh
curl -fsSL https://releases.stim.perish.uk/manage.sh | sh
```

Every non-stable install names one exact version and two isolated paths. For
example:

```sh
version=v0.2.0-beta.1
seat="$HOME/.local/opt/stim-$version"
curl -fsSL https://releases.stim.perish.uk/manage.sh |
  sh -s -- install \
    --channel beta \
    --version "$version" \
    --install-root "$seat/install" \
    --bin-dir "$seat/bin"
```

The root manager and default paths belong to stable. Every channel has an exact
seal at `v1/releases/<channel>/<version>/seal.json`; only stable has a moving
pointer and root manager.
