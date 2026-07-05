# Insights — scope-writing session

- Date: 2026-07-05
- Scope: ../../scope/insights/insights-scope.md (the deliverable of this session)
- Stage: scoping only — **no code changed**; per `SCOPE-WRITTING.md` the implementation
  session creates its own `insights-session.md` when the build starts
- Status: done (scope setup complete)

## Goal

Turn the raw ask — "design an insight system using core things; e.g. a credit-card fraud
alerter and a SkySpark-style HVAC/energy analytics app; is an insight just a term, or do we
need new machinery? should a rule create a channel? is it an outbox message? do we need an
insights page with an AI agent?" — into a complete scope setup per `docs/SCOPE-WRITTING.md`.

## What was done

1. **Surveyed the neighbours** (sub-agent fan-out): `scope/rules/` (emit/alert/Finding,
   messaging handles, approvals), `scope/flows/` (triggers, sinks, node model),
   `scope/ingest/` (+ the just-shipped webhooks), `scope/datasources/`,
   `scope/inbox-outbox/` (+ the crates: `Item` has no severity/dedup/meta; `Effect` lifecycle),
   `scope/channels/` (any principal posts; registry row is cheap; no threading),
   `scope/tags/` (facets, provenance edges, 10k node cap), `scope/agent-personas/` +
   `agent-dock` (the shipped AI surfaces), and `docker/postgres/seed.py` (clean Haystack-tagged
   telemetry, no fault concepts — pure substrate).
2. **Reached the verdict:** ~80% of "insights" is already shipped; the one gap is a durable
   record with severity + dedup/flap-suppression + open/ack/resolve lifecycle + provenance
   (`Finding` is ephemeral; `Item` deliberately stayed minimal; `Effect` is delivery motion).
3. **Answered the design questions decisively** (recorded as rejected alternatives in the
   scope): no channel-per-rule (conversation surface ≠ store; channels stay as explicit sinks);
   not an outbox effect (state vs motion; effects are terminal + unqueryable); not an inbox
   item (no severity/dedup; inbox stays the approval bridge); not zero-new-code (the lifecycle
   needs a mutable record); the primitive is core (rules-cage handles are curated), the
   verticals are extensions/config.
4. **Wrote the scope doc** `scope/insights/insights-scope.md` — record shape, three producer
   doors (rhai handle / flow sink node / MCP verb), consumers (Insights page + agent dock with
   a data-only `builtin.insights-analyst` persona), platform checklist, both worked use cases,
   testing plan (mandatory cap-deny + ws-isolation named), risks, open questions.
5. **Second pass (same day, on review feedback):** the ask grew three key features, each now
   its own sub-scope under the umbrella:
   - `insight-occurrences-scope.md` — "insights should have transactions": one lite (≤2 KB,
     reject-oversize) occurrence row per raise in a per-insight capped ring (default 100,
     `lb-store::capped` primitive), lifetime `count` surviving eviction. Rejected
     occurrences-as-ingest-samples (different cap surface, unbounded, splits the read API).
   - `insight-subscriptions-scope.md` — "subscribe a channel for a rule / all insights / an
     identity" (+ tag facets, the requested tags-for-organising): `insight_sub` record with an
     AND-composed filter (`origin_ref` | `dedup_key` | `tags` | `severity_min` | all),
     raise-time matcher producing intents, delivery under the subscriber's stored principal
     re-checked at fire (reminders pattern), honest dormancy on revoke, 1k/ws cap.
   - `insight-notify-scope.md` — the anti-spam ask: a per-(sub, dedup_key) digest ladder
     (immediate 15-min-cooldown → hourly → daily → weekly → monthly; escalate on noise, decay
     one level per quiet window), breakthrough events (first occurrence, severity escalation,
     re-open) that always deliver, ack-suppression, one-message digests via a
     `react_to_insight_digests` reactor on the injected clock, `insight_policy:{ws}` defaults
     record, per-sub pinned overrides, and a per-member kill switch as a new nullable
     `lb_prefs::Prefs` axis.
   The umbrella was updated to link them (record gains the ring note + `occurrence?` on raise;
   producer-side "explicit authoring only" softened to producer sinks + consumer
   subscriptions).
6. **Scaffolding per SCOPE-WRITTING §7:** created `public/insights/insights.md` (TODO stub),
   added the `insights/` bullet to `scope/README.md`, noted the skill docs the build owns
   (`skills/insights/SKILL.md` + a `core.insights` grounding skill), updated `STATUS.md`.

## Tests

N/A — docs-only session, no code or behavior changed (nothing to run; rule 9 untouched).

## Debugging

Nothing broke — no `debugging/insights/` entries.

## Open for the implementing session

Everything in the scope's "Open questions" (§): alert()→insight coupling, severity taxonomy,
raise quotas, retention default, page↔persona auto-pinning, declarative routing record. The
email outbox `Target` gap is the named prerequisite for the fraud vertical's mail leg.
