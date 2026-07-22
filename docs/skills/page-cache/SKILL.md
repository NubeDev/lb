---
name: page-cache
description: >
  Observe and operate lb's optional server-side response cache over MCP — read hit/miss/eviction
  counters with cache.stats, drop a workspace's cached reads with cache.purge, and understand the
  on/off/budget knobs. Use when tuning page-open latency on an edge node, diagnosing a stale page,
  or confirming the cache is doing its job. Both verbs are admin-only.
---

# page-cache — operating the response cache

lb's response cache (the `page-cache` feature) serves warm page-open reads from memory and coalesces
concurrent identical reads to one compute. This skill drives its two admin verbs and the config knobs.
It is only meaningful on a node built with `--features page-cache` **and** booted with the cache
enabled (`BootConfig.cache = Some(..)`, or `LB_CACHE=1` on the standalone binary); otherwise
`cache.stats` reports `{"enabled": false}` (disabled) or the verbs return `no such tool` (compiled out).

## Prerequisites

- A caller holding `mcp:cache.stats:call` (read) and/or `mcp:cache.purge:call` (admin). Both ride the
  workspace-admin role bundle (`ADMIN_ONLY_CAPS`). A caller without the cap is opaquely `Denied`.
- Reach the verbs over the same MCP surface as any host verb: the gateway `POST /mcp/call` (or an
  in-process `lb_host::call_tool`). The workspace comes from the token, never the request body.

## Read the cache counters — `cache.stats`

Request (no args):

```json
POST /mcp/call
{ "tool": "cache.stats", "args": {} }
```

Response (cache enabled):

```json
{
  "enabled": true,
  "hits": 128,
  "misses": 12,
  "evictions": 0,
  "entry_count": 12,
  "weighted_size_bytes": 4096,
  "per_class": [
    { "class": "series", "hits": 40, "misses": 4 },
    { "class": "datasource", "hits": 88, "misses": 8 }
  ]
}
```

- `hits`/`misses` are node-wide since boot. A single-flight burst shows as **one miss + N-1 hits**.
- `evictions` counts TTL + budget (size) evictions — climbing means the budget is tight for the load.
- `weighted_size_bytes` is the real footprint (key + serialised value bytes) and stays under the
  configured budget. `entry_count` well below your distinct-read count means eviction is working.
- Cache compiled in but disabled at runtime returns `{ "enabled": false }`.

**Confirm the cache is working:** open a page (fires `datasource.list`/`series.list`/`flows.list`/
`ext.list`), read `cache.stats` (misses climb), re-open within 60 s, read again — `hits` climbed and
`misses` did not: the re-open ran zero engine dispatches.

## Drop cached reads — `cache.purge`

Purge is the stale-data escape hatch. It bumps the calling **workspace's** class generations, so every
cached read for that workspace becomes unreachable at once (other workspaces are untouched; memory
frees on TTL/eviction).

```json
POST /mcp/call
{ "tool": "cache.purge", "args": {} }
```

```json
{ "ok": true, "workspace": "acme" }
```

Use it after an out-of-band change the cache cannot see (an external writer to a sqlite datasource, a
manual store edit), or to force a fresh read while debugging.

## What is / isn't cached

Cached (60 s, invalidated on the matching write): `datasource.list`, `series.list`, `flows.list`,
`flows.get`, `ext.list`. Everything else dispatches every call — including **`viz.query`** (deferred:
it is caller-dependent) and all extension `<ext>.<tool>` verbs. A normal write (`ingest.write`,
`flows.save`, `datasource.add`, …) invalidates its class automatically — you do **not** purge after a
write; purge is only for changes the node never saw.

## The knobs (boot-time, not MCP)

- Compile: `--features page-cache` (lb default OFF; a downstream host like rubix-ai turns it on).
- Enable + budget: `BootConfig.cache = Some(CacheConfig { enabled, memory_budget_bytes, list_ttl_secs })`.
  Standalone binary: `LB_CACHE=1`, `LB_CACHE_BUDGET_MB=32`. rubix-ai: on by default, `RUBIX_CACHE_ENABLED=0`
  kill-switch, `RUBIX_CACHE_BUDGET_MB`.

> Grounding: the request/response shapes above are the real verb contracts
> (`crates/host/src/cache/verbs.rs` + `live.rs::stats_snapshot`). A captured live-run transcript lands
> with the rubix-ai smoke test (see the session docs).
