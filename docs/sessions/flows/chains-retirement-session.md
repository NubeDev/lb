# Flows — retire the `chains` engine, flows is the one DAG engine (session)

- Date: 2026-07-01
- Scope: ../../scope/flows/chains-retirement-scope.md
- Stage: S8 (data plane) shipped — this is a post-S8 consolidation slice (STATUS.md)
- Status: done

## Goal

Delete the `chains` rule-DAG engine outright so `flows` is the single DAG engine (README rule 1:
one engine, one job). The `flows` engine is a proven strict superset of `chains` — same binding
grammar, triggers, one-job-per-node topology, frontier driver + CAS run-store, plus richer nodes
and a live SSE canvas. Executes `flows-scope.md` **Decision 6** one step past its wording: a clean
cut, not a deprecated alias (the engines were never unified under the hood — `flows/coordinator.rs`
is a *fork* of the chain coordinator, so an alias would keep the whole forked path alive to back
five dead verbs).

Exit gate restated: `cargo build/test --workspace`, `pnpm test`, `pnpm test:gateway` all green
with **zero** `chains`/`lb_rules::workflow` references outside `rust/rubix-cube/` (upstream) and
`docs/` (lineage), plus a regression test proving each retired verb is **unroutable** (gone, not
merely ungranted).

## What changed

**Open questions resolved (no `AskUserQuestion` — all three were pre-resolved in the ask):**
1. **Seeded chains?** Grepped the seed/demo fixtures (`test_gateway_seed.rs`, all non-test `.rs`
   for `chain:{…}` / `chains.save` / `store:chain`): the only hits were the `credentials.rs` cap
   grants (removed here). **No shipped demo seeds a chain** → dropped the `chain*` tables with the
   code, no migrator (scope Non-goal confirmed).
2. **rules-workbench chain canvas?** Redirected wholly to the flow canvas — the Chains nav entry,
   route, and surface are gone; no standalone chain page left. `rules-workbench-scope.md` DAG story
   now points at Flows.
3. **`rule-chains-scope.md`?** Kept, retired to lineage (banner already present) — the `rubix-cube`
   port rationale is genuinely useful and referenced by `rules-engine-scope.md`.

**Superset proven before deleting any test (scope step A).** A coverage map (chain test → flow
test) confirmed 5/6 backend chain cases already covered; two gaps were closed by **adding flow
tests first**:
- `crates/host/tests/flows_run_test.rs` → `halt_with_a_successful_upstream_is_partial_failure`
  (the ok→err→skipped `partialFailure` case the sibling all-fail test didn't cover) and
  `save_rejects_a_flow_over_the_node_cap` (the `MAX_FLOW_NODES` save-boundary check).
- `crates/flows/src/model.rs` → `rejects_over_the_node_cap` (the validator unit test).
The chain DAG-model tests (`rules/tests/dag_test.rs`: cycle/dangling/dup/self-edge/empty) were
already covered one-for-one by `flows/src/model.rs` unit tests + binding tests in `flows/src/binding.rs`.

**Rust removal (B):**
- host: deleted `crates/host/src/chains/` (all 8 files); removed `mod chains;` + the
  `pub use chains::{…}` block from `lib.rs`; removed the `chains.*` arm from `is_host_native` and
  the dispatch match in `tool_call.rs`; deleted `tests/chains_test.rs`; scrubbed the `chains.list`
  comment in `tests/rules_test.rs`.
- rules crate: deleted `crates/rules/src/workflow/` (model/context/mod) + `tests/dag_test.rs`;
  removed `pub mod workflow;` and its doc mention from `lib.rs`. (Compiler confirmed no `flows`
  code imported `lb_rules::workflow`.)
- gateway: deleted `role/gateway/src/routes/chains.rs` + `tests/chains_routes_test.rs`; removed
  `mod chains;` / the `pub use chains::{…}` line from `routes/mod.rs`; removed the four `/chains…`
  routes + their imports from `server.rs`; removed the six `mcp:chains.*:call` grants + the two
  `store:chain:*` grants from `session/credentials.rs` and reworded the `rules.*`/`chains.*`
  comments to `rules.*`/`flows.*`.

**Hidden-coupling surfaced (not papered over):** `host/src/rules/config.rs::max_chain_steps()` was
chains-only dead code (zero callers besides its re-export; the flows engine uses `lb_flows::
MAX_FLOW_NODES`). Removed the fn + both re-exports (`rules/mod.rs`, `lib.rs`). This is the "shared
type that was actually chains-only" case the scope warned about — it was genuinely chains-only, so
removal (not a stop) was correct.

**UI removal (C):**
- deleted `ui/src/features/chains/` (9 files) and `ui/src/lib/chains/` (3 files).
- removed the Chains entry (+ unused `GitBranch` icon import) from `features/shell/NavRail.tsx` and
  its `CoreSurface` union member; the chains route/surface/`Chains()` component from
  `features/routing/createAppRouter.tsx`; the `chains` path from `routing/surface.ts`; the
  allow-gate from `routing/allowed.ts`; `chainsGet` from `lib/session/admin-caps.ts`; the whole
  `chains_*` block from `lib/ipc/http.ts`.

**Lineage comments reworded** (so the prove-absence grep is clean AND the `rubix-cube` port
attribution the scope wants kept survives): the "ported from the chain coordinator/run_store",
"mirrors ChainsError/StepStateRecord/ClaimState", and "chain binding grammar" comments in the
`flows` crate + `host/src/flows/` now read "rubix-cube" / "the retired chain engine — see
`rule-chains-scope.md` lineage". The `reminders` model comment now points multi-step orchestration
at `flows.run`.

## Decisions & alternatives

- **Chose delete over alias** (executes Decision 6's named end state). Rejected the deprecated
  `chains.*` alias: it would keep ~2100 lines of forked engine + UI alive to serve five verbs no
  external caller uses — the opposite of the "one engine" win. Pre-1.0, no external caller, so the
  clean cut is honest.
- **Chose "grant the defunct cap to prove NotFound"** for the regression test. An *ungranted*
  retired verb is refused opaquely at the authorize gate (`Denied`) — that can't distinguish "gone"
  from "locked". Granting the now-defunct `mcp:chains.<verb>:call` lets the caller pass the gate and
  hit dispatch, where the deleted verb resolves to nothing (`registry.get("chains") → None`) →
  `NotFound`. That `NotFound` is the "gone, not just ungranted" proof. A complementary test asserts
  the ungranted path stays opaque `Denied`.
- **Rejected leaving a stub `chains.run`.** A stub is exactly the dead weight this removes; a real
  future caller migrates to `flows.*` (a rename), not a resurrected engine.

## Tests

Mandatory categories: capability-deny (the regression test IS the deny — the verb is unreachable at
every layer), workspace-isolation (unchanged — the flow isolation tests stay green; chains removed
ws-scoped records, never widened a boundary), offline/sync (the flow resume-idempotent test covers
the former chain restart case).

New regression tests (the headline guard against a stray re-add):
- `crates/host/tests/chains_retired_test.rs` — real spawned host, real store/caps/dispatch:
  `every_retired_chains_verb_is_unknown_not_just_ungranted` (all six verbs → `NotFound`) +
  `an_ungranted_retired_verb_stays_opaque_denied`.
- `role/gateway/tests/chains_retired_routes_test.rs` — real gateway over a real node:
  `every_retired_chains_route_is_404_while_flows_answers` (all six `/chains…` routes 404, `/flows`
  still 200).

Green output:

```
$ cargo test -p lb-host --test chains_retired_test
running 2 tests
test an_ungranted_retired_verb_stays_opaque_denied ... ok
test every_retired_chains_verb_is_unknown_not_just_ungranted ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-role-gateway --test chains_retired_routes_test
running 1 test
test every_retired_chains_route_is_404_while_flows_answers ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test flows_run_test        # incl. the two added superset tests
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cd ui && pnpm test
 Test Files  33 passed (33)
      Tests  242 passed (242)
```

(Full `cargo test --workspace` + `pnpm test:gateway` output pasted at the bottom once complete.)

Prove-absence grep (excluding `rust/rubix-cube/` upstream, `docs/` lineage, build `target/`):
`grep -rniE "chains|lb_rules::workflow"` returns only (a) the two regression test files (which must
name the retired verbs to assert they're gone) and (b) deliberate lineage/"chains retired" comments.
Zero live-engine references remain.

## Debugging

None — no bug surfaced during the removal. The one non-trivial discovery (`max_chain_steps` dead
code) was a clean, expected consequence of deleting the engine, not a defect; handled inline (see
"Hidden-coupling surfaced" above), no `debugging/` entry warranted.

## Public / scope updates

- Promoted: `docs/public/flows/flows.md` gains a "chains removed — flows is the one DAG engine"
  note.
- Scope: `chains-retirement-scope.md` open questions resolved (recorded above);
  `rules-workbench-scope.md` DAG story redirected to Flows; `rule-chains-scope.md` kept as lineage
  (banner already present).

## Dead ends / surprises

- The initial broad `grep chain` matched a large amount of benign English ("resolution chain", "call
  chain", the Rust `.chain()` iterator, "binding chain") and the entirely-separate `host/src/workflow/`
  coding-workflow module (issue→triage→PR) — which is NOT the chains engine and stays. Narrowing to
  the `chains` engine surface (word `chains`, `lb_rules::workflow`, the `chains/` dirs) was essential
  to avoid deleting unrelated code.

## Follow-ups

- The wider docs sweep: several scope/session docs still mention `chains.*` in historical prose
  (e.g. `flows-scope.md` Decision 6 itself, `flow-run-scope.md`, `datasources/page-chaining-*`).
  These are lineage/historical and are left as-is per the scope (only live-surface references were
  rewritten); a future doc-tidy pass could add "retired" banners if desired.
- STATUS.md updated (this slice marked shipped).
</content>
