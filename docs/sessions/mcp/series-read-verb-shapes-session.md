# Session — pin `series.read` wire shapes (issue #60)

Date: 2026-07-14. Branch: `master`. Answers [NubeDev/lb#60](https://github.com/NubeDev/lb/issues/60).
No new lb-core behavior — a **confirmation + test-coverage** task: #56 (keyset paging) and #57
(bucketed decimation) landed in `23fae1eb` with no shape write-up, leaving ems's `fetch-history.ts`
nothing authoritative to code its trend reads against.

## What I did

Traced `series.read`'s single dispatch arm (`crates/host/src/ingest/tool.rs:43-50`) through both its
`mode` branches to the reply serialization and the underlying `PageQuery`/`BucketQuery`/`Cursor`
structs, then to passing tests — extending coverage where a form had none. Added a `series.read`
section to `docs/scope/mcp/ems-provisioning-verb-shapes-scope.md` (same table format #48 used).

Sources read:
- `crates/host/src/ingest/tool.rs:43-180` — the `mode` discriminator (`"rows"` default / `"buckets"`),
  `read_rows`/`read_buckets_mode`.
- `crates/ingest/src/sample.rs:37-58` — the `Sample` envelope (`payload`, not `value`; `ts` is `u64`
  epoch ms).
- `crates/ingest/src/page.rs` — half-open `[from, to)` wall-clock filter vs. inclusive-both-ends
  `from_seq`/`to_seq`; `next_cursor`/`prev_cursor` computation (`Some` only when the page came back
  `limit`-full).
- `crates/ingest/src/bucket.rs:29-44,95-155` — the `Bucket` struct (`t`, not `ts`;
  `min`/`max`/`avg: Option<f64>`; empty buckets omitted, never null-padded).
- `crates/ingest/src/cursor.rs:16-48` — cursor encodes `(seq, producer)`, true keyset (not offset).

## Outcome — ems's bucket-row assumption was half wrong

- Raw/windowed read: row is the full `Sample` envelope (`payload`, `ts` as `u64` ms) — this was never
  pinned before (only `series.latest` was, in #48); confirmed here. Window is half-open `[from, to)`;
  `from_seq`/`to_seq` are inclusive on both ends (an asymmetry worth documenting explicitly).
- Bucketed read: ems assumed `{ t, min, max, avg, last }`. `t` and `last` are right. `min`/`max`/`avg`
  must be read as **nullable** (an all-non-numeric bucket has them `null`), and **empty buckets never
  appear in the array at all** — ems must not render a gap as a zero.
- Keyset paging: confirmed `cursor` in, `next_cursor`/`prev_cursor` out, and — the detail most likely
  to bite a consumer — "no more pages" is `next_cursor: null` (field present), not an absent field.

No contract gaps: all three forms are wired, gated by the single `mcp:series.read:call` cap, and
workspace-isolated. Nothing here required new lb-core behavior.

## Tests

Added the missing wire-level coverage for the raw/windowed form (the only one of the three with no
MCP-surface test before this session — bucketed and paging already had baseline coverage from #56/#57):

- `windowed_read_is_half_open_via_mcp` (`crates/host/tests/series_plane_host_test.rs`) — asserts
  `[from, to)` half-open semantics, the `Sample` envelope on the wire (`payload`/`ts`), and that
  `from_seq`/`to_seq` bound inclusively on both ends (contrast case).
- Extended the existing MANDATORY capability-deny loop in `bucketed_read_via_mcp_and_deny_without_cap`
  to include a windowed (`from`/`to`) input alongside the unbounded and bucketed ones, proving all
  three `series.read` shapes share one cap.

```
cargo test -p lb-host --test series_plane_host_test
```

Result: 5 passed, 0 failed (`paged_read_walks_chain_via_mcp`, `windowed_read_is_half_open_via_mcp`,
`bucketed_read_via_mcp_and_deny_without_cap`, `ws_b_replaying_ws_a_cursor_sees_nothing`,
`retention_round_trip_and_deny_via_mcp`).

## Follow-up

None for lb-core. ems codes `fetch-history.ts`'s `mode: "buckets"` path against the corrected
`min`/`max`/`avg` nullability and the omitted-empty-bucket behavior; the raw-samples path it already
uses needed no change (payload/ts were already read correctly, just previously undocumented for this
verb specifically).
