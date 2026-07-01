# Core scope — the resource-verb convention (one grammar for every listable, runnable thing)

Status: scope (the ask). Promotes to `public/core/core.md` once shipped (adopted by ≥3 families).

Every resource family the platform ships — reminders, jobs, flows, extensions, channels, and the
external-agent runs — has the **same lifecycle shape**: you list them, get one, create/update/delete
them, and (for the ones that *run*) start/stop/inspect them. Today each family invented its own verb
names ad-hoc (`reminder.list` next to `channel_list` next to `installed`), so the command palette, the
`lb` CLI, and agents must learn a different grammar per family. This scope fixes the shape **forever**:
a single canonical verb set (`<resource>.list|get|create|update|delete|watch`, plus a runnable trait
`.start|stop|status|restart|logs`) that every family conforms to, so learning one family teaches all of
them and the palette/CLI render every resource mechanically. It is a **naming + shape convention**, not
a new mechanism — the verbs already dispatch through the one MCP bridge (rule 7); this just makes them
predictable.

## Goals

- **One verb vocabulary.** Define the canonical read/write/run verbs so a new family only decides
  *which* verbs it needs, never *what to call them*.
- **Palette-/CLI-mechanical.** `tools.catalog` groups a family's verbs, so the palette renders
  `/reminder …` the same way it renders `/flow …`; the CLI maps `lb <resource> ls|show|rm|start|stop`
  to `<resource>.list|get|delete|start|stop` with a single dispatch table, not per-family code.
- **Conform the outliers.** Rename the non-conforming shipped verbs to the grammar with a deprecation
  window (`channel_list → channel.list`, `installed → extension.list`), keeping the old name as a
  logged alias for one release so nothing breaks.
- **Fill the CRUD holes** the families are missing but should have (`channel.delete`, `channel.get`,
  `job.list`/`job.get`/`job.cancel`, `extension.get`) — each behind its own capability, one file per
  verb (FILE-LAYOUT).
- **A `--output json` contract** so every `list`/`get` returns the same envelope shape
  (`{ items | item, next_cursor? }`) and the CLI/palette parse one format across all families.

## Non-goals

- **Not a generic CRUD framework or a macro** that auto-generates handlers. Each verb stays a real,
  reviewable function with its own capability check (rule 5). This is a *naming* convention plus a few
  missing verbs — not a code-generator that would hide the caps chokepoint.
- **No new transport, no new registry.** Verbs dispatch through the existing `POST /mcp/call` bridge
  and the existing MCP registry; `watch` reuses the existing gateway SSE (§6.13). Nothing new to route.
- **Not a forced rename of every internal store model.** The convention governs the **MCP verb + CLI**
  surface (what callers and agents see), not private table names (`flow_run`, `channel_registry` stay).
- **No cross-family "list everything" verb.** Each family lists itself; a global search is a separate
  concern (`query/` scope), not this.

## Intent / approach

**The grammar is `<resource>.<verb>`, dot-namespaced, resource singular.** Two tiers:

**Tier 1 — the lifecycle verbs (every listable resource):**

| Canonical verb | CLI sugar | Meaning | Capability |
|---|---|---|---|
| `<r>.list` | `lb <r> ls` | ws-scoped, filterable, **keyset-paged** (`{items, next_cursor}`) | `mcp:<r>.list:call` |
| `<r>.get` | `lb <r> show <id>` | one record by id | `mcp:<r>.get:call` |
| `<r>.create` | `lb <r> create …` | make one; returns the id | `mcp:<r>.create:call` |
| `<r>.update` | `lb <r> update <id> …` | mutate (covers rename/reconfigure/pause-via-field) | `mcp:<r>.update:call` |
| `<r>.delete` | `lb <r> rm <id>` | **soft** by default (undo scope); `--hard` purges | `mcp:<r>.delete:call` |
| `<r>.watch` | `lb <r> watch <id>` | live changes over SSE (where motion exists) | `mcp:<r>.watch:call` |

**Tier 2 — the runnable trait (jobs, flows, extensions, external-agent runs):** a resource that has a
*running lifecycle* adds these on top of Tier 1:

| Canonical verb | CLI sugar | Meaning |
|---|---|---|
| `<r>.start` | `lb <r> start <id>` | begin/arm (flow enable+deploy, ext load, run kick) |
| `<r>.stop` | `lb <r> stop <id>` | halt/disarm (flow disable, ext unload, run cancel) |
| `<r>.status` | `lb <r> status <id>` | one-shot health snapshot (running\|stopped\|failed + summary) |
| `<r>.restart` | `lb <r> restart <id>` | stop+start, one call (supervision) |
| `<r>.logs` | `lb <r> logs <id>` | the run transcript / recent events (bounded tail; `watch` for live) |

**Why a convention and not a trait-macro.** The tempting move is a Rust trait + derive that generates
`list/get/delete` for any record. Rejected: it would centralize the **capability check** into generated
code, exactly the chokepoint rule 5 says must be explicit and reviewable per verb. The house rule is
"one verb per file, one cap per verb" (FILE-LAYOUT); a convention keeps that while still giving the
palette/CLI a predictable surface. The uniformity we want is in the **names and the JSON envelope**, not
in shared handler code. (The palette already proves this works: `tools.catalog` groups by the `group`
descriptor field — the convention just standardizes what those groups are called.)

**Pause/enable is `update`, not a bespoke verb — with named CLI sugar.** A reminder pauses via
`reminder.update {enabled:false}`; a flow arms via `flows.enable`. To keep the grammar tight, the
**canonical** control is `update` (or the runnable `start`/`stop`), and `flows.enable`/`disable` become
**CLI/palette aliases** that expand to `flows.start`/`stop` — one control verb, friendly names on top.

**The rename is aliased, not breaking — and the alias never expires on the wire (D1).** `channel_list`
and `installed` are renamed to `channel.list` and `extension.list`; the old names stay registered in a
**permanent** alias map (`registry/aliases.rs`) that resolves old→canonical **forever** and emits one
`deprecated_verb` audit line on use. What *is* time-bounded is only the **advertisement**: after one
release `tools.catalog` stops returning the old name, so the palette/CLI/help surface is clean and no new
caller learns it — while any durably-stored caller (a saved reminder action `{tool:"channel_list"}`) keeps
dispatching via the permanent alias. Cut the surface, keep the compatibility. The old **capability**
grant is honored permanently by the same map for the same reason. This is stricter than
`chains-retirement`'s hard cut *because a stored action is data we own indefinitely* — see D1.

## How it fits the core

- **Tenancy / isolation:** unchanged — every verb is already ws-scoped; the convention doesn't touch the
  wall. The mandatory isolation test per family still applies to each new verb (`channel.delete` in ws-B
  can't delete a ws-A channel). Aliases resolve *before* the cap check, so an alias can't dodge isolation.
- **Capabilities:** each canonical verb keeps its **own** capability (`mcp:<r>.<verb>:call`); a rename
  renames the cap too, with the **old cap grant honored as an alias** during the deprecation window so a
  workspace's existing grants don't silently lose access on upgrade. The deny path is unchanged: no
  `<r>.delete` cap → `delete` refused, opaque. **Mandatory deny test** per new verb.
- **Placement:** either — pure host code over the registry + store; no `if cloud`. The runnable verbs
  (`start`/`stop`) act on whatever node hosts the resource (a flow's placement, an ext's tier), which is
  already the family's own concern, not this convention's.
- **MCP surface (API shape §6.1):** this scope *is* an API-shape convention. It names the four shapes per
  family: **CRUD** (`create/update/delete`), **get/list** (`get` + paged `list`), **live-feed**
  (`watch` over SSE where motion exists; N/A for a static resource — say so), **batch** (a bulk
  `delete`/`update` that can run long is a **job**, per §6.1 — e.g. "delete all done reminders" enqueues
  an `lb-jobs` job and returns a job id, it does not loop in the handler).
- **Data (SurrealDB):** no new tables. The convention governs verb names; records keep their existing
  private table names. `list` uses the **shipped keyset cursor** (`page-cursor-scope.md`) so every
  family pages identically (`{items, next_cursor}`), not a per-family paging shape.
- **Bus (Zenoh):** no new subjects. `watch` reuses each family's existing motion subject and the
  existing gateway SSE route; a family with no motion has **no** `watch` (state-vs-motion, rule 3) and
  the convention says so rather than inventing an empty stream.
- **Sync / authority:** node-local verb dispatch, unchanged. A soft `delete` is a normal
  workspace-authoritative record write that syncs on the normal path (undo scope owns the tombstone).
- **Secrets:** none — verb names carry no secret material; a `get` redacts secret-bearing fields exactly
  as the family's read verb already does (§6.7).
- **Stateless:** the alias table is static registry config, not durable per-instance state — hot-reload
  safe.
- **One responsibility per file (FILE-LAYOUT):** each new verb is one file (`channel/delete.rs`,
  `job/list.rs`, …). The alias map is one file (`registry/aliases.rs`). The CLI dispatch table is one
  file (`cli/src/dispatch.rs`, which already exists — extended, not duplicated). **No `verbs.rs` grab-bag.**
- **SDK/WIT impact:** **none on the guest ABI.** Extension-declared tools already carry a `name`; the
  convention only *recommends* extensions name their verbs `<id>.<verb>` (they already do — `<id>.<tool>`).
  No manifest field changes. Flag: the `tools.catalog` `group` field should carry the resource name so the
  palette groups correctly — additive, already present.

## Example flow

An operator cleans up finished reminders and checks a flow, all through the uniform grammar:

1. `lb reminder ls` → CLI calls `reminder.list` → the paged `{items, next_cursor}` table. Same shape as
   `lb flow ls` (`flows.list`) and `lb ext ls` (`extension.list`, formerly `installed`).
2. `lb reminder rm rmd_7f3` → `reminder.delete {id, hard:false}` → soft-deleted (tombstoned, undoable).
   `--hard` would purge. A principal without `mcp:reminder.delete:call` gets an opaque deny — the CLI
   prints the header + a stderr error + non-zero exit, no record touched.
3. "Delete every `done` reminder" → `lb reminder rm --all --status done` → because this is unbounded, the
   host **enqueues an `lb-jobs` job** and returns a job id; the CLI prints `queued job job_9a2` and the
   operator `lb job watch job_9a2`. No blocking loop in the handler (§6.1).
4. `lb flow status flow_42` → `flows.status` → `running, 3 nodes armed, last settle 2s ago`. `lb flow
   stop flow_42` → `flows.stop` (alias of `flows.disable`) disarms it; `lb flow start flow_42` re-arms.
5. In the browser, typing `/reminder` in a channel opens the **same** verb set from `tools.catalog`
   (grouped `reminder`), capability-filtered — the palette and the CLI are provably the same grammar.

## Testing plan

Per `scope/testing/testing-scope.md`; real store/bus/gateway, seeded records, no mocks (§0).

- **Capability-deny (mandatory):** each *new* verb (`channel.delete`, `channel.get`, `job.list`,
  `job.get`, `job.cancel`, `extension.get`) denies opaquely without its cap. An **alias** honors the
  old cap grant during the window and denies without it after — assert both.
- **Workspace-isolation (mandatory):** ws-B cannot `list`/`get`/`delete`/`stop` a ws-A resource via any
  new or renamed verb; a ws-B `job.list` never returns ws-A jobs. Mirror the existing per-family
  isolation tests.
- **Alias correctness:** `channel_list` and `installed` dispatch to the canonical handler, emit exactly
  one `deprecated_verb` audit line, and return the identical result as the canonical name. After the
  window (feature-flag the cut in the test), the alias is absent → clean `UnknownTool`.
- **Envelope conformance (unit):** every family's `list` returns `{items, next_cursor?}` and every `get`
  returns `{item}` — one table-driven test across the families asserts the shape, so a family can't drift
  its JSON contract.
- **Batch-is-a-job:** an unbounded `delete`/`update` returns a **job id**, not an inline result, and the
  job runs the deletions durably (survives a restart mid-batch). Assert the handler does *not* loop.
- **CLI dispatch (integration):** `lb <r> ls|show|rm|start|stop|status` map to the right verb for each
  family via one dispatch table; `-o json` yields the uniform envelope. Real gateway (rule 9).
- **Palette grouping (UI, real gateway):** `tools.catalog` groups the renamed verbs under the resource
  name; `/reminder`, `/flow`, `/ext` each open the family's verb set, capability-filtered.

## Risks & hard problems

- **Rename blast radius.** `channel_list`/`installed` are called from the UI, tests, and possibly saved
  flows/reminders (an MCP-tool action storing `{tool:"channel_list"}`). The alias window is the
  mitigation, but **stored** references (a reminder whose action calls the old name) must resolve through
  the alias at fire time too — test a reminder created against the old name after the rename.
- **Cap-grant migration.** Renaming `mcp:installed:call → mcp:extension.list:call` must not silently drop
  a workspace's grant. Honor the old cap as an alias during the window; document the one-release cut so
  admins re-grant before it's removed. Getting this wrong locks a workspace out of listing its extensions.
- **Over-standardizing.** Not every family needs every verb. A channel has no meaningful `start/stop`; a
  reminder has no `logs`. The convention is a **menu, not a mandate** — a family declares the subset it
  needs and states why the rest are N/A (exactly this doc's own discipline). Forcing empty verbs is worse
  than omitting them.
- **Soft-delete semantics vary.** `flows.delete` today may be hard; `reminder.delete` may be immediate.
  Unifying on soft-default touches the undo scope's tombstone model per family — sequence this *after*
  undo is settled, or scope soft-delete per family, don't retrofit it blindly in one pass.
- **`status` vs `watch` overlap.** `status` is a snapshot; `watch` is a stream. Easy to conflate. Keep
  `status` a bounded one-shot (cheap, pollable) and `watch` the SSE — don't let `status` grow into a
  poll-loop that duplicates the bus feed.

## Decisions (v1)

All resolved — long-term-correct, no interim hacks. The build session implements these; record any
deviation in the session doc.

- **D1 — Alias resolution: permanent for the wire, one-release for the surface.** The rename splits into
  two lifetimes, because a *stored* reference and an *advertised* name are different hazards:
  - **Dispatch alias — permanent.** `channel_list → channel.list` and `installed → extension.list` are
    registered in a permanent `registry/aliases.rs` map that resolves old→canonical **forever**. A
    reminder/flow that durably stored `{tool:"channel_list"}` must never silently break on upgrade — a
    saved action is data we're on the hook for indefinitely, so the dispatch alias never expires. Using an
    alias emits one `deprecated_verb` audit line (observability, not breakage).
  - **Catalog/palette advertisement — one release.** After one release the **catalog stops advertising**
    the old name (`tools.catalog` returns only canonical), so the palette/CLI/help surface is clean and no
    *new* caller learns the old name — while any *existing* stored caller keeps working via the permanent
    dispatch alias. This is the honest version of "clean cut": cut the surface, keep the compatibility.
  - **Cap-grant alias — permanent, same reasoning.** A workspace's `mcp:installed:call` grant keeps
    authorizing `extension.list` via the same permanent map, so an upgrade never locks a workspace out of
    listing its own extensions. New grants are issued under the canonical cap; the old cap is honored, not
    minted. Rejected: a timed cap cut (a silent lockout waiting to happen on some release nobody tracked).
- **D2 — Soft-delete rollout: per-family, gated on `undo/` for that family.** `delete` is soft-default
  *where the family has a settled tombstone model*, adopted per-family as `undo/` lands for it — not a
  blind one-pass retrofit that would half-wire tombstones. The convention **mandates the `delete` verb +
  the `--hard` flag shape now**; the soft-vs-hard *default* flips to soft per family as undo settles. A
  family whose undo isn't ready ships `delete` as hard with `--hard` implied and says so — no fake soft
  path pretending to be undoable.
- **D3 — `list` filter grammar: shared minimal core + optional per-family fields.** Every `list` takes
  `{status?, limit, cursor}` (so the CLI offers `--status/--limit` and keyset paging uniformly across all
  families); a family MAY add its own typed filter fields (`kind?` on jobs, `channel?` on reminders). One
  base shape, family extensions additive — the CLI/palette render the common controls everywhere and the
  extras where declared.
- **D4 — `create` returns the id (+ the canonical get-path), not the full record.** Keeps `create` cheap
  and the envelope uniform (`{id}`); a caller that wants the full record `get`s it. Rejected: returning the
  whole record — it bloats `create`, invites callers to skip `get` and then drift when the record changes
  server-side, and makes the create/get envelopes inconsistent.
- **D5 — CLI: first-class per-family subcommands, generated from one dispatch table.** `lb reminder ls`,
  `lb job cancel <id>`, `lb flow status <id>` — real clap subcommands per family (discoverable, tab-
  completable, good `--help`), all wired from a **single** kind→verb dispatch table in `cli/src/dispatch.rs`
  so there's one source of truth and no per-family hand-copied plumbing. Rejected: a generic
  `lb resource <family> <verb>` — it reads worse, loses per-family help/completion, and hides the grammar
  the convention exists to make legible.

## Related

- `scope/channels/channels-command-palette-scope.md` — `tools.catalog` groups verbs by the `group`
  descriptor field this convention standardizes; the palette is the primary UI consumer.
- `scope/channels/channels-edit-delete-scope.md` — the `channel.delete`/`.get` this convention fills in.
- `scope/reminders/reminders-scope.md` — already conforms (`reminder.list|get|create|update|delete`); the
  reference family.
- `scope/flows/flow-runtime-control-scope.md` — the runnable trait (`flows.enable|disable|cancel|watch`)
  this generalizes; `flows.enable/disable` become `start/stop` aliases.
- `scope/jobs/jobs-scope.md` — the missing `job.list|get|cancel` read/control surface (jobs are a scan
  today, no verbs); the batch-is-a-job backing for unbounded `delete`/`update`.
- `scope/extensions/lifecycle-management-scope.md` — the `start·stop·enable·disable·install·delete`
  lifecycle this names uniformly; `installed → extension.list`.
- `scope/external-agent/run-lifecycle-scope.md` — `agent.runtimes` (list) + `agent.watch`; the run is the
  runnable trait applied to a foreign loop.
- `scope/datasources/page-cursor-scope.md` — the keyset `{items, next_cursor}` every `list` reuses.
- `scope/undo/` — the soft-delete tombstone model `delete` defaults to.
- `rust/role/cli/src/dispatch.rs` + `cli.rs` — the one CLI dispatch table this convention feeds.
- `README.md` §3 (rules 5/7), §6.1 (API shape), §6.5 (MCP), §6.13 (gateway SSE).
- `public/core/core.md` — promotion target on ship.
</content>
</invoke>
