# Insights scope — occurrences (the per-insight transaction log)

Status: scope (the ask). Sub-scope of [`insights-scope.md`](insights-scope.md); promotes to
`public/insights/` with it.

An insight is deduplicated on `(ws, dedup_key)` — one row per *thing we know* (a card, an AHU,
a meter). But the individual firings matter: the analyst looking at "card 4421 flagged" wants
the last N transactions with their scores; the engineer looking at "AHU-2 short-cycling" wants
the last N cycle measurements. This scope adds a **bounded, lightweight occurrence log** under
each insight — the "transactions" — without turning the insight table into a second time-series
store.

## Goals

- Every `insight.raise` appends **one occurrence row** under the (possibly pre-existing)
  insight: the firing log is complete within its bound.
- Occurrences are **lite by construction**: a small per-firing delta (score, value, txn ref),
  never a repeat of the parent's title/origin/tags, with a **hard size cap that rejects**
  oversize payloads (the page-context 4 KB-reject precedent).
- Storage is a **capped ring** per insight — keep the last N, drop-oldest — so a
  fires-every-5-minutes insight costs O(N), not O(forever).
- The parent's `count`/`first_ts`/`last_ts` remain the **lifetime** truth; the ring is the
  recent evidence. `count` MUST be allowed to exceed the stored rows.

## Non-goals

- **Not a time-series plane.** High-frequency raw data stays in ingest `series`; an occurrence
  is one row *per raise*, and raises are already dedup-shaped. If a producer wants full
  history, it writes a series and the occurrence carries the pointer.
- **No occurrence-level lifecycle.** Ack/resolve live on the parent only.
- **No occurrence editing/deletion.** Append-only within the ring; the ring itself evicts.

## Intent / approach

Reuse the platform's existing capped-ring primitive (`lb-store::capped`, named by the
telemetry-console scope; `flow_node_buffer` is the flows precedent) rather than inventing
eviction. One new record family + one read verb + one optional field on `raise`.

Rejected alternative: occurrences as ingest samples on a per-insight series (`insight:{ws}:{id}`
as a series id). Rejected because samples are workspace-visible under a different read cap
(`mcp:series.read:call`), immutable-unbounded (retention is the series plane's own open
problem), and it splits one feature's read surface across two verb families. The ring keeps
insights self-contained; a producer that *also* wants series history writes both explicitly.

## The record

```
insight_occ:{ws}:{insight_id}:{seq}
{
  seq,            // monotonic per insight (host-assigned)
  ts,             // logical ts of the raise
  severity,       // the severity THIS firing carried (parent holds the latest)
  data?           // opaque JSON delta — score, reading, txn ref, evidence link
}
```

- **Size cap:** `data` serialized ≤ **2 KB**, enforced at `raise` — oversize is a `BadInput`
  rejection of the whole raise (never silent truncation; the producer must slim its payload or
  store evidence elsewhere and link it).
- **Ring cap:** default **100 per insight**; workspace-admin adjustable within hard bounds
  `[0, 1000]` (0 = occurrences disabled for the workspace — raise still works, nothing stored).
  The cap lives on the notify/insights workspace policy record (see
  [`insight-notify-scope.md`](insight-notify-scope.md)), not per-raise.
- **Parent linkage:** the parent's `count` increments on every raise (lifetime, monotonic);
  `last_ts` and `severity` take the newest firing's values. When `count > ring size`, the UI
  shows "count 4,312 — last 100 shown".

## Verb surface

- **`insight.raise`** (existing verb, additive field): optional
  `occurrence: { data?, severity? }`. Whether or not the field is present, **every raise
  appends one occurrence row** (an empty one is still the firing log); `occurrence.severity`
  defaults to the raise's top-level `severity`.
- **`insight.occurrences { id, cursor?, limit? }`** → newest-first keyset page of the ring
  (page-cursor contract). Own verb, own cap `mcp:insight.occurrences:call` — the list/get read
  cap does not imply evidence access (evidence may be more sensitive than the headline).

## How it fits the core

- **Tenancy:** rows keyed under `{ws}`; ring scan is ws + insight scoped.
- **Capabilities:** append rides `mcp:insight.raise:call` (no separate cap — an occurrence
  cannot exist without a raise); read is `mcp:insight.occurrences:call`, deny opaque.
- **Data:** SurrealDB rows via the capped-ring primitive; state only, no bus motion beyond the
  parent raise event (which gains `count`/`seq` so live UIs can update without a re-fetch).
- **Placement:** either; no reactor.
- **API shape:** write = implicit in `raise` (no standalone create); read = keyset list;
  no update/delete; batch N/A.
- **Skill doc:** covered by `skills/insights/SKILL.md` (umbrella) — the raise example must show
  a lite `occurrence.data` and the size-cap rejection.

## Example flow

1. Rule fires for card 4421: `insight.raise(#{ dedup_key: "fraud:4421", severity: "critical",
   occurrence: #{ data: #{ score: 0.93, txn: "t-88123", amount: 412.50 } } })`.
2. Same card, an hour later, score 0.71 → same insight, `count: 2`, ring now holds two rows.
3. After 150 firings: `count: 150`, ring holds the newest 100.
4. Analyst opens the detail drawer → `insight.occurrences` pages the ring newest-first; each
   row is the score + txn ref, not a copy of the insight.

## Testing plan

- **Capability deny (mandatory):** `insight.occurrences` denied without its cap even when the
  caller holds `insight.get`; raise-with-occurrence denied without `insight.raise`.
- **Workspace isolation (mandatory):** ws-B cannot read ws-A occurrences; seq spaces are
  per-insight per-ws.
- **Ring semantics:** cap+1 raises → cap rows, oldest evicted, `count` = cap+1; cap=0 workspace
  stores nothing but `count` still increments.
- **Size cap:** 2 KB+1 `data` → whole raise rejected `BadInput`; exactly-2 KB accepted.
- **Severity:** occurrence severity recorded per firing; parent reflects the newest.
- **Paging:** newest-first keyset over >1 page.

## Risks & hard problems

- **"Lite" is a discipline, not a wall.** 2 KB × 1000-cap × many insights is still real bytes;
  the workspace ring cap is the honest lever, and the retention follow-up (umbrella scope)
  must include occurrence rows.
- **Ring eviction vs audit expectations.** Users may assume the log is complete; the UI and
  skill doc must surface `count` vs stored explicitly.

## Open questions

1. Should the ring cap be settable **per insight** at first raise (a producer knows fraud needs
   500, HVAC needs 20), or is the workspace default + hard bounds enough for v1? (Recommend:
   workspace-only v1.)
2. Should an occurrence be able to carry a `series` pointer as a first-class field (typed link
   to raw data) rather than inside `data`? (Recommend: inside `data` until a consumer needs to
   traverse it.)

## Related

- [`insights-scope.md`](insights-scope.md) (umbrella — the parent record),
  [`insight-notify-scope.md`](insight-notify-scope.md) (the workspace policy record that hosts
  the ring cap)
- `scope/observability/telemetry-console-scope.md` (the `lb-store::capped` ring primitive),
  `flow_node_buffer` in `scope/flows/` (the flows capped-ring precedent)
- `scope/datasources/page-cursor-scope.md` (keyset paging contract)
