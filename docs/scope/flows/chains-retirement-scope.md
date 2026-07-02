# Flows scope — retire `chains`, flows are the one DAG engine

Status: scope (the ask). Promotes to `public/flows/flows.md` once shipped (the "chains removed"
note). **Read the spine first:** [`flows-scope.md`](./flows-scope.md) — this doc executes its
**Decision 6** ("`flows.*` is the general surface; `chains.*` becomes its rule-only special case")
one step further than written: **delete `chains` outright** rather than keep it as a permanent
alias.

We shipped two DAG engines. `chains` (the rule-DAG: `chains.*` MCP verbs, the
`WorkflowCoordinator`, the `lb_rules::workflow` model, a React chain canvas) and `flows` (the
node-graph engine: `flows.*`, its own frontier `coordinator`, the React Flow canvas). **`flows` is
a strict superset of `chains`** — same binding grammar (lifted verbatim), same `manual|cron|event`
triggers, same one-`lb-jobs`-job-per-node topology, same frontier driver + CAS run-store, plus
`Subflow`/`Sink`/`Source` nodes, retained inputs, and the data-driven canvas. A chain is nothing
but a flow whose nodes are all `Rhai`/`Tool` steps. So `chains` is **dead weight that duplicates a
proven engine** — we remove it and make `flows` the single home for "chain rules into a DAG."

## Goals

- **Delete the `chains` surface entirely:** the `chains.*` MCP verbs, the host `chains` module, the
  gateway `chains` routes, the `lb_rules::workflow` DAG model, the React chain feature, and the
  `mcp:chains.*` cap grants — no compatibility alias, a clean cut (nothing external depends on it;
  the whole platform is pre-1.0, in-dev — `flow-message-envelope-scope.md` already takes breaking
  cuts on the same posture).
- **Make `flows` the sole, documented way to build a rule-DAG.** After removal, "chain rules"
  means "author a flow of `Rhai`/`Tool` nodes" — same capability, one engine.
- **Fold the still-canonical engine detail into the flow docs.** The DAG-math, binding-resolver,
  run-store shape, and failure-policy prose in `rule-chains-scope.md` that flows *ships* stays
  discoverable — as flow-engine documentation (`flow-run-scope.md`), not a second surface's scope.
  `rule-chains-scope.md` is retired to lineage/history ("ported from `rubix-cube`, generalised into
  flows"), not deleted (the `rubix-cube` attribution + port rationale is worth keeping).
- **Green after removal:** `cargo build/test --workspace` and `pnpm test` pass with every `chains`
  reference gone; a **regression test** asserts the retired verbs are unroutable.

## Non-goals

- **Removing `lb-rules` (the single-rule engine).** `rules.*` — the `rhai` cage, the `Grid`, the
  verb library — **stays**; a flow `Rhai` node runs a rule through it. Only the `workflow/` DAG
  **module inside** the rules crate goes (it existed solely to drive chains). See
  [`../rules/rules-engine-scope.md`](../rules/rules-engine-scope.md) — untouched.
- **Any new flow capability.** This is a pure removal + doc-reconcile; it adds no `flows.*` verb, no
  node kind, no schema. If a chain did something a flow can't, that would be a finding for a flow
  scope — but none exists (superset verified below).
- **A data migration of saved chains.** Nothing has shipped saved `chain:{ws}:{id}` records in a
  real workspace (the feature landed in the rules-workbench slice, exercised only by gateway tests
  seeding their own records). We drop the `chain*` tables with the code; no migrator. If that
  assumption is wrong at build time, see Open questions — the fallback is a one-shot
  `chain`→`flow` transcode, not keeping the engine.
- **Touching `flows` execution semantics.** The flow coordinator already re-ported the chain
  frontier driver verbatim; we don't refactor it, we just remove the original it was forked from.

## Intent / approach

**Delete, don't alias — because "alias" would preserve the duplication we're removing.** Decision 6
hedged toward a thin `chains.*` alias delegating to the flow engine, for callers mid-migration. But
the two engines never got unified: `flows/coordinator.rs` is a **fork** of `chains/coordinator.rs`
(its own header says "ported from the chain `coordinator`"), not a caller of it. An alias would
therefore mean *keeping the whole chain code path alive* to back five deprecated verbs — the
opposite of removing dead weight. Since the whole platform is pre-1.0 and no external caller exists,
we take the clean cut Decision 6 named as the eventual end state and do it now.

**The superset is proven, so deletion loses nothing.** Mapping every chain concept to its flow
equivalent (this is the load-bearing claim — if any row had no flow home, we could *not* delete):

| `chains` concept | `flows` equivalent | Same code? |
|---|---|---|
| `Chain` (DAG of `Step`s) — `lb_rules::workflow::model` | `Flow` (DAG of `Node`s) — `lb_flows::Flow` | Flow re-ported the model; chain model is now unused-elsewhere. |
| `Step { rule_id, needs, with }` | `Rhai`/`Tool` node + edges | A step *is* a rule-node; a `Tool` node generalises it to any verb. |
| `${steps.x.output}`/`${params.y}` bindings | identical grammar on flow ports | **Lifted verbatim** (`node-descriptor-scope.md` "binding grammar"). |
| `WorkflowCoordinator` (`start`/`on_step_done`, frontier, `Halt`/`Continue`) | flow `coordinator` (`start`/`drive`, same frontier + policy) | Flow's is a **fork** of chain's — same logic. |
| `chain_run` / `chain_step_output` (CAS run-store) | `flow_run` / `flow_step_output` (same CAS shape) | Same design (Decision 8). |
| Triggers `Manual\|Cron\|Event` | flow triggers `manual\|cron\|event\|inject\|boot` | Flow **superset** (adds inject/boot). |
| `chains.run/save/get/list/runs.get` MCP | `flows.run/save/get/list/runs.get` MCP | 1:1 verb map (+ `flows.watch`, `flows.patch_run`, …). |
| chain canvas (`ChainCanvas`, `StepNode`, poll `runs.get`) | flow canvas (`FlowCanvas`, `FlowNodeView`, `flows.watch` SSE) | Flow **superset** (schema-form palette, live SSE). |

Every row has a flow home; several rows are flow **improvements**. There is no chain capability
without a flow expression. Deletion is safe.

**Rejected — keep `chains.*` as a deprecated alias (Decision 6 as-written).** It reads safer but
is strictly worse here: it keeps ~2100 lines of forked engine + UI alive to serve five verbs no one
calls, so the "one engine, one surface" win (rule 1) never actually lands and the fork rots. The
alias made sense *if the engines had been unified under the hood*; they weren't. With no external
caller and a pre-1.0 posture, the clean cut is the honest execution of Decision 6.

**Rejected — refactor `flows` to call a shared chain engine instead of deleting.** The reverse
merge (make flows' driver a thin wrapper over the chain coordinator) would also collapse the
duplication, but backwards: it re-centres the retired name as the core and drags the narrower
`Chain`/`Step` model into the general engine's spine. Flows is already the general shape with the
richer node model; we keep the general engine and delete the special case, not the other way round.

## How it fits the core

- **Symmetric nodes (rule 1) — the whole point:** two engines for one job is the duplication rule 1
  exists to forbid. Removal leaves **one** DAG driver (`flows`), config-and-role only, no branch.
- **One datastore (rule 2):** the `chain*` tables are dropped with the code; `flow*` tables remain
  the single home for DAG run-state. No new persistence — this only *removes*.
- **MCP is the contract (rule 7):** the `flows.*` family is the one surface; the retired `chains.*`
  verbs become **unroutable** (a call returns the host's unknown-verb deny). No verb is orphaned —
  each retired verb has a live `flows.*` twin (map above).
- **Capabilities:** the six `mcp:chains.*:call` grants are removed from `credentials.rs` and the
  admin-caps set. A regression test asserts a `chains.*` call is refused **as unknown**, not merely
  ungranted (the verb is gone, not just locked).
- **Tenancy / isolation:** unchanged and untouched — `flows` already enforces the workspace wall on
  every `flow:{ws}:{id}` record; removing `chains` removes ws-scoped records, it never widens a
  boundary. The mandatory isolation tests live in the flow sub-docs already.
- **Placement:** N/A — nothing placement-sensitive changes; both engines were `either`.
- **MCP surface (§6.1):** this scope **removes** surface, adds none. The retained read/write/run
  verbs are all `flows.*`, already scoped in the sibling docs. No new CRUD/list/watch/batch.
- **SDK/WIT impact:** none — no manifest/WIT change. `chains` never had a `[[node]]` block or a WIT
  world; it was host verbs + a rules-crate module + UI. Pure host-and-frontend removal.
- **One responsibility per file (FILE-LAYOUT):** removal deletes whole single-responsibility files
  (`chains/save.rs`, `chains/run.rs`, `workflow/model.rs`, `ChainCanvas.tsx`, …); it leaves no
  half-file remnant and introduces no `misc`/`utils` catch-all.

## The removal surface (the concrete work-list)

Every site that references the retired engine, grouped by layer. The implementing session works
this list to zero, then greps for `chain` to confirm nothing survives outside `rubix-cube` (the
upstream source tree) and the retired-to-lineage `rule-chains-scope.md`.

**Rust — host (`rust/crates/host/`):**
- Delete `src/chains/` (all 8 files: `mod.rs`, `coordinator.rs`, `record.rs`, `run.rs`,
  `run_store.rs`, `save.rs`, `get.rs`, `error.rs`).
- `src/lib.rs`: remove `mod chains;` and the `pub use chains::{…}` re-export block.
- Remove the `chains.*` dispatch arms wherever the host router matches them (they live in the
  deleted `chains/mod.rs`; confirm no second dispatch site references them).
- Delete `tests/chains_test.rs`; scrub the stray `chains.list` comment in `tests/rules_test.rs`.

**Rust — rules crate (`rust/crates/rules/`):**
- Delete `src/workflow/` (`model.rs`, `context.rs`, `mod.rs`) — it exists **only** for chains
  (verified: no `flows` code imports `lb_rules::workflow`).
- `src/lib.rs`: remove `pub mod workflow;` and its doc-comment mention.

**Rust — gateway (`rust/role/gateway/`):**
- Delete `src/routes/chains.rs`; in `src/routes/mod.rs` remove `mod chains;` + the `pub use
  chains::{…}` line.
- `src/server.rs`: remove the four `/chains…` route registrations and the `list_chains,
  save_chain, …` imports; keep the sibling `/flows…` routes.
- `src/session/credentials.rs`: remove the six `mcp:chains.*:call` grants and reword the
  `rules.*`/`chains.*` comments to `rules.*`/`flows.*`.

**UI (`ui/src/`):**
- Delete `features/chains/` (9 files) and `lib/chains/` (3 files).
- `features/shell/NavRail.tsx`: remove the Chains nav entry.
- `features/routing/{surface.ts,createAppRouter.tsx,allowed.ts}`: remove the chains route/surface
  and its allow-gate.
- `lib/session/admin-caps.ts`: remove `chainsGet` (`mcp:chains.get:call`).
- `lib/ipc/http.ts`: remove any `chains` endpoint path constant.

**Docs (see §7 below):** retire `rule-chains-scope.md` to lineage; rewrite the ~10 docs that
cross-reference `chains.*` to point at `flows.*`; drop the `chains.*` line from the scope README and
key-stack; note the removal in `public/`.

## Example flow

The user-visible before/after, and the safety of the cut.

1. **Before:** a workspace admin opens **Chains** in the nav, builds a rule-DAG on the chain canvas,
   `chains.save`s it, `chains.run`s it, and watches steps colour via a `chains.runs.get` poll. The
   same admin *also* has **Flows** in the nav doing the strictly richer version of the same thing.
2. **The cut lands.** The Chains nav entry, route, canvas, `lib/chains`, the host `chains` module,
   the gateway `/chains` routes, the `lb_rules::workflow` model, and the `mcp:chains.*` grants are
   all deleted in one change. `cargo build --workspace` and `pnpm test` are green.
3. **After:** the admin builds the same rule-DAG in **Flows** — drop `Rhai`/`Tool` nodes, wire them
   with the *same* `${steps.x.output}` bindings, pick a `cron` trigger, `flows.run`, watch it settle
   over `flows.watch` (live SSE, an upgrade on the old poll). Same capability, one engine.
4. **Deny/regression path:** a client that still hard-codes `chains.run` gets the host's
   **unknown-verb** deny (the verb no longer exists) — not a silent 500, not a partial execution. A
   regression test asserts exactly this for each retired verb.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`) — **no mocks**: real store (`mem://`), real
gateway, real dispatch. This is a *removal*, so the tests prove **absence is clean** and **the
superset still covers the use case**, not new behavior.

- **Regression — retired verbs are unroutable (the headline):** a real spawned host + gateway; a
  call to each of `chains.save`/`chains.run`/`chains.get`/`chains.list`/`chains.runs.get`/
  `chains.delete` returns the **unknown-verb** deny (verb gone, not just ungranted). Same for the
  `/chains…` gateway routes → 404/not-found. This is the new `debugging/`-worthy guard against a
  stray re-add.
- **Superset coverage stays green (the equivalence proof):** the flow gateway/E2E suites
  (`FlowsCanvas.gateway.test.ts`, `FlowsRuntimeControl.gateway.test.ts`,
  `FlowDashboardBinding.gateway.test.ts`) — which exercise a `Rhai`/`Tool`-node DAG with
  `${steps.x.output}` bindings, a `cron`/`event` trigger, and a live settle feed — **remain green**,
  demonstrating every retired chain path has a live flow path. If a flow test *doesn't* already
  cover a former chain-only case, **add it before deleting the chain test** (never delete coverage
  net-net).
- **Workspace-isolation (mandatory, unchanged):** the flow isolation tests (ws-B cannot
  get/run/watch a ws-A flow) stay green; no isolation test is lost, because the chain isolation
  tests are replaced one-for-one by their flow equivalents (which already exist).
- **Capability-deny (mandatory):** with `mcp:chains.*` grants removed, the regression test above
  *is* the deny test (the verb is unreachable at every layer); the flow deny tests
  (`mcp:flows.<verb>:call` required) are the live equivalents and stay green.
- **Build gate:** `cargo build --workspace` + `cargo test --workspace` + `pnpm test` all green with
  zero `chains`/`lb_rules::workflow` references remaining (a `grep` step in the session is the
  proof), excluding the `rubix-cube` upstream tree and the retired-to-lineage scope doc.
- **Debug entry (if anything breaks):** if removal surfaces a hidden coupling (e.g. a shared type
  that turned out not to be chains-only), log a `docs/debugging/flows/<symptom>.md` entry, fix, add
  a regression test, and update `docs/debugging/README.md` — per the session rules.

## Risks & hard problems

- **A hidden coupling into `lb_rules::workflow` or `chains`.** The plan asserts nothing outside
  `chains` imports `workflow` — verified by grep at scope time, but the compiler is the real proof:
  delete, then let `cargo build` name any straggler. Low risk, mechanically findable.
- **Losing coverage on delete.** The failure mode is deleting a chain test that covered a case no
  flow test does, silently narrowing coverage. Mitigation is the testing-plan rule: **add the flow
  test first, then delete the chain test** — coverage never dips through the change.
- **Docs drift.** ~10 docs reference `chains.*`; a missed one leaves a dangling link to a retired
  surface. Mitigation: the §7 list is exhaustive (grepped), and the self-check re-greps.
- **Under-estimated: the "alias felt safer" pull.** The temptation mid-build is to leave a stub
  `chains.run` "just in case." Resist — a stub is the dead weight this scope removes. If a real
  caller surfaces, it migrates to `flows.*` (a rename), not a resurrected engine.

## Open questions

- **Are there seeded `chain:{ws}:{id}` records in any shipped demo/seed path** (not just
  test-local)? Grep the seed/demo fixtures. If **yes**, add a one-shot `chain`→`flow` transcode in
  the removal session (a `Rhai`-node flow per chain step) so no saved artifact is orphaned; if
  **no** (the expected answer), drop the `chain*` tables with the code and skip the migrator.
- **Does the `rules-workbench` frontend scope keep a chain canvas as a distinct page, or does its
  DAG story now redirect wholly to the flow canvas?** Expected: redirect to Flows; confirm with the
  `rules-workbench-scope.md` owner so the nav/route removal doesn't strand a documented page.
- **Keep `rule-chains-scope.md` as a lineage doc, or delete it?** Recommendation: **keep, retitled**
  as history (the `rubix-cube` port rationale + attribution is genuinely useful and referenced by
  `rules-engine-scope.md`); mark it "superseded by `flows/` — not a shipping surface" at the top so
  no one mistakes it for live scope.

## Related

- [`flows-scope.md`](./flows-scope.md) — **Decision 6** (this doc executes it) and **Decision 8**
  (the shared topology that makes chains redundant).
- [`flow-run-scope.md`](./flow-run-scope.md) — the flow run-store + frontier driver that *is* the
  chain engine, generalised; the fold-in home for the retained engine prose.
- [`node-descriptor-scope.md`](./node-descriptor-scope.md) — the binding grammar lifted verbatim
  from chains; the `Rhai`/`Tool` node that replaces a chain `Step`.
- [`../rules/rule-chains-scope.md`](../rules/rule-chains-scope.md) — **retired to lineage** by this
  scope (the `rubix-cube` port history; superseded as a shipping surface by `flows/`).
- [`../rules/rules-engine-scope.md`](../rules/rules-engine-scope.md) — **untouched**; `lb-rules`
  (the single-rule engine) stays; only its `workflow/` DAG module goes.
- [`../frontend/rules-workbench-scope.md`](../frontend/rules-workbench-scope.md) — the chain canvas
  UI this removes; its DAG story redirects to the flow canvas (Open question 2).
- README §3 (rule 1 — one engine, the whole justification), §6.5 (MCP surface), §6.10 (jobs — the
  one topology both used).
</content>
