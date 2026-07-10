---
name: e2e-inbox-outbox-demo
description: >
  Seed an energy/insights-themed INBOX + OUTBOX demo into a running node — real inbox items (some
  needs:approval), resolutions, and must-deliver outbox effects through the real host verbs, so a
  demo has data to SHOW on the channel view + outbox/proof panel. Docker-free, idempotent.
---

# Inbox + outbox demo seed — energy alerts to show for a demo

Status: demo seed (real-world). Design intent:
[`../../scope/inbox-outbox/inbox-outbox-scope.md`](../../scope/inbox-outbox/inbox-outbox-scope.md)
(the inbox half) and
[`../../scope/inbox-outbox/outbox-scope.md`](../../scope/inbox-outbox/outbox-scope.md)
(the transactional must-deliver outbox). Runbook it plugs into:
[`../e2e-backend.md`](../e2e-backend.md).

This is the messaging sibling of the datasource seed
([`../datasources/README.md`](../datasources/README.md)): it gives a demo **real records to look
at** on the inbox channel view and the outbox/proof panel. Everything is written through the **real
host verbs** the UI itself uses (`inbox.record` / `inbox.resolve` / `outbox.enqueue` over
`POST /mcp/call`) — no fakes (rule 9). The energy story lines up with the seeded `demo-buildings`
dataset so the alerts read as if they were raised off that data (kWh/kW/L·min/temp).

## Run it

```bash
make dev                 # boot the node first
make seed-demo-energy    # or: bash docs/testing/inbox-outbox/seed-demo-energy.sh
```

Idempotent — every write is a stable-id upsert, so re-running replaces the same rows, never dupes.
Logs in as `user:ada`/`acme` (an admin — holds the author caps `inbox.record` / `outbox.enqueue` /
`inbox.resolve`; a bare `viewer` deliberately does not).

## What it seeds (workspace `acme`)

**Inbox** — channel `energy-ops`, 7 items ordered by logical `ts` (oldest→newest):

| id | gist | resolution |
|----|------|-----------|
| `energy-1` | HQ-North demand spike 412 kW (+18% vs peak) | — |
| `energy-2` | B4 chiller CH-2 efficiency drift (kW/ton 0.94) | — |
| `energy-3` | WM-07 cooling-tower makeup — suspected leak | — |
| `energy-4` | **[needs:approval]** demand-response event (shed 60 kW) | **approved** |
| `energy-5` | daily energy report ready (8 sites, 4,910 kWh) | — |
| `energy-6` | **[needs:approval]** after-hours HVAC runtime | **deferred** |
| `energy-7` | **[needs:approval]** solar export dropped to 0 kWh | **rejected** |

The three `needs:approval` items carry a real `Resolution` sibling record (approve / defer / reject)
— the approve/reject/defer surface the workflow reads, not just body text.

**Outbox** — 4 `pending` must-deliver effects (the durable follow-through):

| id | target | action | payload gist |
|----|--------|--------|--------------|
| `eff-notify-dr` | `email` | `notify` | facilities: DR event dispatched (−60 kW) |
| `eff-workorder-b4` | `workorder` | `create` | high-priority chiller work order |
| `eff-report-daily` | `report` | `publish` | publish the daily energy report |
| `eff-notify-leak` | `email` | `notify` | facilities: suspected WM-07 leak |

(No target adapter is wired for these targets in a bare dev node, so they stay `pending` — which is
exactly what you want to *show*: durable, must-deliver, not-yet-delivered intents.)

## Read it back (prove it landed — real read verbs)

```bash
BASE=http://127.0.0.1:8080
TOKEN=$(curl -fsS -X POST $BASE/login -H 'content-type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)

# inbox items, oldest→newest
curl -fsS -X POST $BASE/mcp/call -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"tool":"inbox.list","args":{"channel":"energy-ops"}}' | jq

# outbox: pending / delivered / dead_lettered / held buckets
curl -fsS -X POST $BASE/mcp/call -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d '{"tool":"outbox.status","args":{}}' | jq
```

Verified live on 2026-07-10: 7 items land in `ts` order, the 3 resolutions record, and all 4
effects show up in `outbox.status.pending` with their payloads intact.

## Notes

- `ts` is a **logical** timestamp (monotone per channel — no wall-clock in core, testing §3). The
  demo uses small increasing integers for deterministic ordering; the channel view sorts on it.
- **Workspace-scoped:** everything lands in `acme`. Seed a second workspace by passing it as the 3rd
  arg (`… seed-demo-energy.sh $BASE user:bob globex`) — a `globex` `inbox.list` never returns
  `acme`'s items (the workspace wall).
- The `author` of each item is forced to the caller's principal (`user:ada`), never the `--arg`
  input — you can't spoof it (see `tool_call.rs`).
