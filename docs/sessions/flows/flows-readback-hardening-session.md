# Session: flows read-back hardening (deploy value consistency + multi-port audit)

- Date: 2026-07-15
- Ask: "not 100% consistent getting values back after it's deployed; check that all nodes can have
  more than one input (same as node read) and many outputs as well; review and improve/harden."
- Scope grounding: `docs/scope/flows/flow-persistent-runtime-scope.md` (the read-back contract),
  `flow-input-ports-scope.md` + `flow-plain-wiring-scope.md` (the port model).

## What the review found

### 1. The read-back inconsistency has a concrete cause: one scan page treated as the whole table

Every flows verb that reads a shared per-ws table and filters in code called
`lb_store::scan(…, MAX_SCAN_LIMIT, None)` **once** — one 200-row page — and never followed
`page.next`. Ten call sites: `node_state.rs` (last-values + retained inputs), `run_store.rs`
(`scan_run_slots` — the drive frontier AND `finalize_if_complete` — plus both retained-input
readers), `runs.rs` (`runs.list` + the `runs.get` snapshot), `save.rs` (`flows_list_internal`,
which also feeds the cron/source reactors), `concurrency.rs` (the live-run guard), and
`orphan_sweep.rs`. Under 200 rows per table everything is exact — which is why tests and a fresh
deployment are green — and past it, values silently vanish from `flows.node_state`, runs stop
finalising (or mis-settle), `Skip` concurrency stops seeing live runs, and reactors can skip flows.
Full record: `docs/debugging/flows/single-scan-page-drops-rows-past-200.md`.

### 2. A failed firing left a stale "current" value

`record_outcome` upserted `flow_node_state` **only on Ok**; an `Err` firing updated the step record
but left the node's last-value record holding the previous good value with no error indication — a
canvas reading `flows.node_state` showed a broken node as healthy forever.

### 3. Multi-input / multi-output: confirmed working, by design

- **Many inputs:** any node accepts any number of wires (`needs` fan-in). Since `flow-plain-wiring`
  every port joins `any` by default — the node fires once per arriving message (Node-RED
  semantics), each firing under its own minted `fctx`; `all` (barrier) is an explicit
  extension-descriptor opt-in. Covered by `flows_run_test` / `flows_multi_trigger_test`.
- **Many outputs:** an output fans out to every wired dependent unconditionally
  (`release_dependents`); `switch` routes per-rule via `config.rules[].to`.
- **By-design edges worth knowing (not bugs, documented in the scopes):** a *barrier* node with ≥2
  inputs requires an explicit `payload` binding (the save lint enforces it — no silent null);
  non-`payload` metadata (`topic`, …) does not carry across an explicit-binding join with ≥2
  upstreams (D4: no ambiguous merge); an `any`-funnel node firing N times per run keeps only the
  last firing's value in `flow_node_state` (Decision 5: last-value read — per-firing history lives
  on the step records / a series).

## What shipped

- `rust/crates/host/src/flows/scan_all.rs` — the cursor loop that drains every scan page; all ten
  flows call sites moved onto it. Full-drain-then-filter (the scan cursor is the `⟨⟩`-bracketed
  `<string>id` rendering whose ordering disagrees with the display id, so a prefix-seeded early
  exit would be unsound); flows tables are retention-bounded so the drain stays small.
- `record_outcome` (`run_store.rs`): an `Err` firing now **merges** `lastError` onto the node's
  `flow_node_state` record — the last good envelope stays readable next to the error; the next Ok
  overwrites the whole record, clearing it. `flows.node_state` lifts it to a per-node `error`
  field for the canvas badge.
- Regression tests: `rust/crates/host/tests/flows_scan_paging_test.rs` — 240 real seeded rows past
  the page boundary for node_state and step slots (rule 9: real records into the real store), plus
  the Err→`lastError` and cleared-on-next-Ok contracts via a real `json` node runtime failure.
  The error contract is deliberately TWO tests, not one ok→err→ok saga: the three-run sequence
  overflowed the default 2 MiB debug test stack (poll-chain depth, not recursion, and not the new
  Err write — bisected and exonerated in a clean worktree). Full record:
  `docs/debugging/flows/three-run-e2e-sequence-overflows-default-test-stack.md`.

## Open follow-ups (named, not silent)

- **Filed as [#69](https://github.com/NubeDev/lb/issues/69).** The same single-page pattern exists
  outside flows: `rules/get.rs`, `dashboard/store.rs`,
  `panel/store.rs`, `report/store.rs`, `nav/store.rs`, `brand/store.rs`,
  `render_templates/store.rs`, `insight/notify.rs` — each passes its own cap but `scan` clamps to
  200 regardless. Sweep them onto a shared drain (or a store-level `scan_prefix`).
- A store-level prefix scan (cursor seeded at the prefix, server-side `string::starts_with`) would
  turn the flows full-drains into O(prefix) reads if a workspace ever profiles hot. Deliberately
  not built now: the store crate has concurrent in-flight work (online compaction), and correctness
  was the ask.
- Per-firing (`any`-funnel) read-back keeps last-write-wins per Decision 5; if a canvas ever needs
  all N firings of one run, `flows.runs.get` already exposes the per-`fctx` slots.
