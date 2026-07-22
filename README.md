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
reply-only listener uses a second, unrelated bearer. Turn events are consumed by cursor
backfill; SSE is only a wake-up, so reconnects do not lose replies.

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
  "label_prefix": "stim:",
  "credential_sha256": "<64 lowercase hexadecimal characters>"
}
```

Generate a distinct reply token and place only its digest in `[reply]`:

```sh
openssl rand -hex 32
printf '%s' "$STIM_REPLY_TOKEN" | sha256sum
```

`listen.address` serves the main user API and defaults to loopback. Put it behind the
user identity boundary before exposing it. `reply.address` is the cross-host listener;
it exposes only `POST /api/v1/replies` and verifies the reply bearer by digest. Terminate
TLS at the host edge; the service listeners themselves speak HTTP.

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
`SANTI_STRAND_ID` and `SANTI_TURN_ID`; configure the remote endpoint and the separate
reply token there:

```sh
export STIM_BASE_URL='https://stim.example.com:43309'
export STIM_REPLY_TOKEN='<raw reply token>'
stim reply 'I am still working'
```

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
negentropy constitution. Release assets are installed through the R2-backed
`manage.sh`, which verifies the published checksum before extraction. Publishing
remains inert until the repository's `STIM_RELEASES_*` variables and secrets are
provisioned.
