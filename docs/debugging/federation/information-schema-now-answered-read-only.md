# Stop steering, start answering — `information_schema` probes are now served read-only

**Area:** federation (query engine) — supersedes the steering half of
[../agent/federation-information-schema-probe-cryptic-plan-error.md](../agent/federation-information-schema-probe-cryptic-plan-error.md)
**Date:** 2026-07-05
**Symptom:** Even WITH the steering rejection in place, live GLM runs kept probing
`SELECT … FROM information_schema.tables` (it is the first move of every OpenAI-schooled model) and
burned turns on the rejection. Fighting the model's strongest prior was the wrong side of the
trade.

## Fix (decision: answer the probe, don't reject it)

- The sidecar validator (`extensions/federation/src/validate.rs`) classifies instead of rejecting:
  `information_schema.tables` / `information_schema.columns` are flagged (never registered as user
  tables); `pg_catalog.*` and unknown `information_schema` views still reject with a steer naming
  the supported views + `federation.schema`.
- New `extensions/federation/src/info_schema.rs` synthesizes the two views per query as in-memory
  tables built from the source's REAL catalog (the same `list_tables` / provider-schema reads
  `federation.schema` uses), registered under an `information_schema` schema in the per-query
  `SessionContext`. Strictly read-only, ephemeral, never a passthrough to `pg_catalog`.
  DataFusion's built-in information-schema was rejected as the mechanism: it only reflects
  *registered* tables, and we register only what a query references — it would always be empty.
- The host gate (`crates/host/src/federation/validate.rs`) lets the two views through (it rejected
  them before the sidecar could answer); `pg_catalog` still rejects with the steer.

## Regression tests

Sidecar validator units (flag-not-collect + unsupported-steer) and host-gate units
(allow information_schema / reject pg_catalog) — green.

**Verified live:** `SELECT table_name FROM information_schema.tables` returns the source's real
tables; the widget-builder run used it naturally and proceeded straight to a working query.
