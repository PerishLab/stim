# Cross-host protocol

## Authority split

Santi is authoritative for runtime execution. Stim is authoritative for participants
and message delivery. The only shared identifier semantics are Santi's opaque external
label and durable turn identifiers.

Stim owns the `stim:` zone. A participant `operator` maps to `stim:operator`. Santi
validates only that the authenticated downstream stays inside the registered prefix.

## Inbound to Santi

Stim stages a human message in its own database before calling `POST /api/v1/ingest`.
The request ID is globally unique within the downstream and is reused verbatim after an
ambiguous network result. Santi either returns the original receipt or rejects a changed
payload using the same key.

The returned strand ID is bound to the participant in stim's database. This mapping is
what lets a later early-reply request identify the participant without asking Santi to
interpret the label.

If the network result is ambiguous, the staged message stays pending and an independent
worker retries it with the same request ID. A stalled ingest cannot block turn-event
backfill.

## Outbound from Santi

Stim reads `GET /api/v1/turn-events?since=<cursor>` with its downstream bearer. Santi
filters payloads to the registered zone and returns a global opaque high-water cursor.
Stim commits the batch and cursor together. Foreign event volume may be inferred from
cursor movement, but foreign labels and payloads are never disclosed.

`GET /api/v1/turn-events/stream` is a lossy wake-up only. On startup, reconnect, and
every wake, stim drains cursor backfill until caught up.

## Early reply

The reply listener exposes only `POST /api/v1/replies`. Its bearer is distinct from the
Santi downstream bearer and is stored by stim only as a SHA-256 digest. Requests carry
the Santi-injected strand and turn identifiers plus content.

Turn ID is the idempotency key. An exact explicit replay returns the original message.
A changed explicit payload conflicts. An automatic completion after an explicit reply
is consumed without replacing the explicit message; otherwise the completion becomes
the automatic reply and deduplicates by turn ID.

An early reply can beat the ingest receipt across hosts. Until the strand-to-participant
mapping is known, stim durably holds that authenticated reply as pending. Receipt
acceptance or the first completion event binds the strand and materializes the pending
reply in the same transaction.
