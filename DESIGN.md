# Design

## Authority split

Santi is authoritative for runtime execution. Stim is authoritative for
participants, the durable message ledger, and delivery. Their only shared
identifier semantics are Santi's opaque external label and durable turn IDs.

Stim owns the `stim:` zone. A participant such as `operator` maps to
`stim:operator`; Santi only verifies that the authenticated downstream stays
inside the registered prefix.

## Inbound

Stim stages a human message in its own database before calling Santi ingest.
The request ID is globally unique within the downstream and is reused after an
ambiguous network result. Santi returns the original receipt for an exact
retry and rejects a changed payload under the same key.

The receipt binds the returned strand ID to the participant in Stim's estate.
That mapping lets a later early reply identify the participant without asking
Santi to interpret the label. An ambiguous ingest remains pending and an
independent worker retries it; stalled ingest never blocks event backfill.

## Outbound

Stim reads turn events with its downstream bearer. Santi filters payloads to
the registered zone and returns an opaque global high-water cursor. Stim
commits each batch and cursor together. Cursor movement may reveal foreign
volume, but foreign labels and payloads are never disclosed.

The event stream is a lossy wake-up only. Startup, reconnect, and every wake
drain cursor backfill until caught up, so disconnects cannot lose replies.

## Early reply

The reply listener exposes only the reply endpoint. Its short-lived Ed25519
capability is distinct from the Santi downstream bearer and binds the issuer,
audience, shell origin, strand, turn, tool call, and effect. Every request
field must equal its signed claim.

Turn ID is the idempotency key. An exact explicit replay returns the original
message; a changed explicit payload conflicts. An automatic completion after
an explicit reply is consumed without replacing it. Otherwise the completion
becomes the automatic reply and deduplicates under the same turn.

An authenticated early reply may beat the ingest receipt across hosts. Stim
holds it durably as pending until the strand-to-participant mapping arrives.
Receipt acceptance or the first completion event binds the strand and
materializes the pending reply in the same transaction.
