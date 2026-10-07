# Agent guide

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure. That document owns
organization-wide Issue, pull-request and acceptance policy; this file keeps
repository-specific constraints without copying that policy.

`stim` is a standalone message product downstream of santi. It owns message identity,
participants, persistence, delivery projection, and its own HTTP/CLI boundary. Santi
owns souls, strands, turns, and provider execution.

## Layering

The shape this repository is being built toward, and the reason the boundaries
below exist:

```
keel                    resource graph / six verbs / trigger / grants / projections
  ^
stim lib                the single implementation of business capability
  ^
stim api                the one surface those capabilities are exposed through
  ^
web | cli | winui | swiftui | ...        display panes, which only carry interaction
```

- **The CLI is a display pane**, not a privileged entrance. It is simply the
  thinnest and least lossy one — it maps almost 1:1 onto the api, so it covers
  every capability, while a UI covers a subset because interaction design costs
  surface. An agent reaches for the CLI because it is the lowest-loss pane, not
  because it has a private road. No pane bypasses the api.
- **Interaction goes through hooks.** A view must not hold `fetch`,
  `EventSource`, or an event listener of its own. Everything a pane can do
  arrives through its hooks layer, which is why auditing hooks is the same as
  auditing the whole interaction surface.
- **Anything expressible only in one UI framework is a shape error.**
  Capability is defined once, below the panes; the web pane's Svelte binding on
  `@perishlab/design` is a binding, never a definition. WinUI and SwiftUI panes
  are a matter of time, and they will consume the same api.

These are prose for now, deliberately: the shape is not settled enough to
mechanize, and a wall written down is the enforcement until a check lands.

## Boundaries

- `crates/core` owns the durable ledger and wire models. It performs no network or CLI
  I/O.
- `crates/cli` owns the `stim` binary, HTTP server/client, config, and the cross-host
  Santi consumer.
- Santi is reached only through its public generic downstream API. Never open or infer
  Santi's database.
- The label zone is `stim:`. Santi treats it as opaque; stim alone interprets the
  participant suffix.
- The Santi downstream bearer and Santi-signed Stim reply capability are
  distinct authorities.
- Outbound delivery is cursor backfill first; SSE is only a wake-up. Explicit early
  replies win over the automatic completion for the same turn.

## Operating

- Never commit on `main`; use an Issue-anchored topic worktree and land through
  the repository guard.
- Copy `stim.example.toml` to ignored `stim.toml`, then provide the configured
  Santi downstream token through its named environment seat.
- `sidecar start --config sidecar.toml` owns the local service lifecycle;
  `cargo run --locked -p stim -- service serve` runs the same service directly.
- Register only the downstream token digest with Santi. Keep the reply signing
  key out of both estates and rotate public keys by `kid` across the maximum
  capability lifetime.
- Required checks: `cargo fmt --all --check`, `cargo clippy --locked --workspace
  --all-targets -- -D warnings`, `cargo test --locked --workspace`, and
  errors-only `ectropy .`.
- Do not deploy, publish releases, create credentials, or mutate live services without
  explicit operator authorization.

## Release

- `plumb.toml` is the product-owned release declaration. Releases follow
  Plumb's lifecycle (`plumb release --help`) and are distributed by wharf; the
  repository holds no release credential.
- A stable's changelog goes to the Depot; this repository carries no release
  notes of its own.
