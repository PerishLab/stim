# Agent guide

`stim` is a standalone message product downstream of santi. It owns message identity,
participants, persistence, delivery projection, and its own HTTP/CLI boundary. Santi
owns souls, strands, turns, and provider execution.

## Boundaries

- `crates/core` owns the durable ledger and wire models. It performs no network or CLI
  I/O.
- `crates/cli` owns the `stim` binary, HTTP server/client, config, and the cross-host
  Santi consumer.
- Santi is reached only through its public generic downstream API. Never open or infer
  Santi's database.
- The label zone is `stim:`. Santi treats it as opaque; stim alone interprets the
  participant suffix.
- The Santi downstream bearer and the stim reply bearer are distinct credentials.
- Outbound delivery is cursor backfill first; SSE is only a wake-up. Explicit early
  replies win over the automatic completion for the same turn.

## Operating

- Never commit on `main`; use a topic branch and `runseal :land`.
- Keep `.task/` ignored unless a long-running local task needs it.
- Required checks: `cargo fmt --all --check`, `cargo clippy --locked --workspace
  --all-targets -- -D warnings`, `cargo test --locked --workspace`, and
  `negentropy --strict .`.
- Do not deploy, publish releases, create credentials, or mutate live services without
  explicit operator authorization.
