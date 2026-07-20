# Packs scope — domain packs in core (the `pack.*` verb family: one declaration turns a blank workspace into a product)

Status: **SHIPPED** — 2026-07-18 (`crates/packs` + `crates/host/src/pack/`, the `pack.*` verb
family; session: `docs/sessions/packs/pack-core-session.md`; issue NubeDev/lb#79). Live-verified:
blank workspace → one `pack.apply` → a real FDD insight raises. The downstream prototype is deleted
(NubeIO/rubix-ai#13) — the engine has exactly one home. Implementation decisions the PR made and
documented: bundle encoding + an 8 MiB cap, the `lb-packs` crate name, `pack.apply` admin-only with
validate/list/get at viewer, and a **sqlite-only, in-process materializer** for a pack's datasource
(the one place an apply touches the node filesystem — the tradeoff and the follow-up federation
`exec_sql` question are recorded in the session doc). Owning repo: **lb (core)**. Origin: proven
end-to-end downstream first — rubix-ai built and live-verified a full applier over public
verbs (`NubeIO/rubix-ai` `crates/pack-apply`, issue NubeIO/rubix-ai#13); this scope moves
that proven engine INTO core so every embedder gets it, and the downstream prototype is
deleted. The format, refusal matrix, and receipt semantics below are not speculation — they
shipped and were live-verified against a real node.

A node boots blank and generic — that is the point. But a *deployment* never is: it is a
building-automation system, an EMS, a product-management tool. A **domain pack** is one
versioned, declarative artifact — datasource schema + optional seed, the semantic
**vocabulary** (entities, kinds, units, insight-key grammar), rules, pre-bound dashboards,
channels, and the AI agent's domain context — applied to ONE workspace. Today only rubix-ai
can do this, via a downstream applier that had to rediscover verb shapes and store quirks
from outside. Core owning the engine makes "blank node + one call = a working product" a
platform capability: same wall, same caps, same symmetry as every other subsystem.

## Rule 10, stated up front

Core owns the **mechanism** and knows **no pack by name**. `bas` vs `ems` differ only in
data; packs are authored and shipped by embedders and third parties, never in this repo
(demo fixtures excepted, under `demo/`). No `if pack == "..."` anywhere; a pack grants
nothing the caller's caps don't already grant; applying is mediated per underlying object by
the exact authority the equivalent verb would demand. A generic, domain-blind pack engine is
what rule 10 *permits*; a named pack in core is what it forbids.

**Naming collision, called out:** `lb-pack` (`rust/tools/pack`) is the *extension artifact*
packager/signer — unrelated. This scope's surface is the **`pack.*` MCP verb family**
(namespace verified free) and a `crates/host/src/pack/` verb module + a pure-logic lib
crate; nothing here touches the extension toolchain.

## Goals

- **The `pack.*` verb family**, workspace-scoped and caps-walled, on the one MCP dispatch:
  - `pack.validate` — parse manifest + files, return the ordered object plan + checksums +
    lint findings (errors gate, dialect warnings don't). This IS the dry-run and the CI
    validator.
  - `pack.apply` — idempotent apply with the **proven refusal matrix** (below), per-object
    outcomes, loud clobber listing, receipt written atomically at the end.
  - `pack.list` / `pack.get` — first-class receipts (replacing the downstream
    `store.query`-on-`pack_receipts` convention with a real, caps-walled read surface).
- **The manifest format** (ported as proven): `pack.yaml` + files-by-reference — `entities`
  (vocabulary: label/parent/kinds/units; explicitly unstable until a runtime consumer),
  `insights.keys` (dedup-key grammar), `datasource` (schema/seed SQL), `rules` (Rhai),
  `dashboards` (cell JSON), `channels`, `agent.context` (markdown → workspace agent memory),
  `extensions` (requirements checked against `ext.list`, never installed), `sidebar.hidden`
  (workspace hidden-set → `nav.hidden.set`, the first **workspace-seed** block — below).
- **Bundle-over-the-wire**: `pack.apply`/`pack.validate` take the manifest + referenced
  files as one checksummed bundle in the call (size-capped). No node-filesystem coupling —
  a third party can apply a pack over MCP with nothing but a session and caps, which keeps
  "MCP is the contract" literally true. A node-local path convenience can ride the CLI, not
  the verb.
- **Receipts as records**: `{pack, version, manifest_checksum, applied_ts, objects: [{kind,
  id, checksum, outcome}]}` — workspace-scoped, store-backed internally (no public-store
  envelope quirks; the downstream `SELECT data FROM …` workaround dies here).
- **Internal seams, not verb-chaining**: `pack.apply` calls the same internal functions the
  public verbs mediate (rules save, dashboard save, datasource add, channel create, agent
  memory set) — one transaction-ish orchestration with per-object authority checks equal to
  the public verbs'. The verb-name folklore a downstream applier had to discover
  (`datasource.add` vs `register`, `agent.memory.set` vs `config.set`) ceases to exist as a
  failure mode.

## Non-goals

- **No pack authoring/marketplace in core.** Distribution is a later scope — plausibly the
  extension artifact toolchain (`lb-pack` signing/registry) grows a pack artifact kind; not
  this ask.
- **No upgrade/rollback mechanics** (v1→v2 diffs): the refusal matrix forecloses nothing
  (receipts carry per-object ids + checksums), mechanics come later.
- **No templating** in manifests: plain YAML + files. Computed seed ships as a generator
  outside the pack.
- **No UI in this repo** (the shell is downstream): viewers/builders are embedder surfaces
  riding `pack.list`/`pack.get`.

## The proven semantics (ported, not re-litigated)

Live-verified downstream and carried over verbatim:

- **Refusal matrix**: first apply → applies + runs rules once; same version + same
  content-checksum → idempotent NoOp; same version + changed content (checksum folds every
  referenced file, not just `pack.yaml`) → refuse "bump the version"; higher version →
  refuse "upgrade not built"; lower → refuse "downgrade". A prior **partial** receipt
  (cap-denied object) at the same version re-applies — recovery is "grant the cap, re-run";
  idempotence is what makes that safe.
- **Clobber rule**: re-apply CLOBBERS pack-owned objects, loudly — the result lists every
  overwrite (agent context is the sharpest edge; never silent). Skip-if-modified is the
  named later upgrade (per-object checksums make it cheap), not v1.
- **Rules run on first apply only** (the receipt knows) — idempotence must not depend on
  every rule's dedup key.
- **Object identity**: receipts record stable ids (rule id = filename stem, dashboard id,
  channel name, datasource name), never display names — drift detection depends on it.
- **Dialect lint**: the DataFusion∩SQLite poison list (CTEs, `datetime()`, scalar
  subqueries) is a WARNING in `pack.validate`, not a gate — the real oracle is the apply.

## Workspace-seed kinds (the closed-`Kind` extension path)

`pack.apply` dispatches on a **closed `Kind` enum** (`crates/packs/src/plan.rs`) — there is no
plugin seam. A new `pack.yaml` block is a new object kind, and adding one is a fixed four-step
recipe, all on data (rule 10 holds — every arm branches on the KIND, never on a named pack):

1. a `Kind` variant + its stable `as_str()` string (the receipt's `kind`, a reader's discriminator);
2. a **plan arm** (`plan()`): the id + a checksum over the block's bytes, so a changed block is drift;
3. a **dispatch arm** (`apply.rs`): deserialize the block and call the SAME internal function the
   equivalent public verb calls — which **re-checks its own capability** under the caller's principal;
4. the **manifest serde** for the block (`deny_unknown_fields`, line-numbered errors).

Nothing else changes: `apply_plan`/the receipt/the refusal matrix are generic over the plan. The
per-object caps wall, clobber listing, and partial-recovery all apply for free.

**Shipped: `Kind::Sidebar`** (`node-v*`, first of the workspace-seed family). Block:

```yaml
sidebar:
  hidden: [channels, datasources]   # refs in the shared nav grammar; opaque data
```

- **Plan**: one object per workspace, keyed by the pack name (like `agent`), checksum over the
  ordered ref set. A changed set is drift (re-applies), same set is a NoOp.
- **Dispatch**: `apply_sidebar` calls `nav_hidden_set` — full-set LWW, so a re-apply **clobbers
  loudly** (listed as `sidebar:<pack>`), exactly the shipped clobber contract.
- **Caps**: `nav_hidden_set` authorizes `nav.save` under the caller's principal — a caller who
  can't shape the workspace's menus by hand can't hide via a pack either. Declutter, **never
  authz**: a hidden surface's route still loads by deep link (the gateway re-checks every verb).

**Planned (this family, not yet built):** `Kind::Nav` (`nav.save`/`set_default`/`share`),
`Kind::Team` + `Kind::User` (create-if-absent, **never-clobber** for people — the one place the
loud-clobber rule inverts). See the consumer scope for the manifest shapes and the re-apply
asymmetry that makes people-kinds a distinct arm.

## Intent / approach

- `crates/host/src/pack/` — the verb family module (folder-of-verbs like `agent/`,
  `dashboard/`): dispatch, per-verb handlers, receipt store access. One responsibility per
  file.
- A small pure lib (manifest/plan/decision — parse, checksum, refusal matrix) with the unit
  tests ported from the downstream crate's 14 (`serde_yaml` line-numbered errors,
  `deny_unknown_fields`, the matrix table).
- Caps: `mcp:pack.validate:call` (read-tier), `mcp:pack.apply:call` (admin-tier — it writes
  through every object family), `mcp:pack.list|get:call` (member read — receipts are
  operator documentation). Per-object authority is ALSO checked inside apply, equal to the
  public verbs' — a caller who couldn't `rules.save` can't smuggle a rule in via a pack.
- Symmetric nodes: nothing role-gated; a pack applies wherever the workspace lives.
- Tests, the house bar: real node (`mem://` store, real gateway), no mocks — the **demo
  oracle** ports too: blank node → one `pack.apply` (the `bas` fixture from the downstream
  port, living under `demo/`) → a real insight raises. Cap-deny → partial receipt →
  grant → re-apply recovery. Workspace-wall: two packs in two workspaces, no cross-reads.

## Embedder migration (rubix-ai, the prototype donor)

On the tag that ships this: rubix-ai deletes its `crates/pack-apply` engine (manifest/plan/
decision/receipt/apply/gateway), keeps its packs (`packs/bas|ems|product-management`) as
data + a thin CLI that bundles files and calls `pack.validate`/`pack.apply`, and its Packs
UI reads `pack.list`/`pack.get` instead of `store.query`. Tracked downstream in
NubeIO/rubix-ai#13; the downstream scope defers engine ownership to this document.

## Risks & hard problems

- **Bundle size/limits**: seeds can be large; cap the bundle (declared limit, honest error)
  and keep "big seed = generator script, not pack payload" the doctrine.
- **Apply is not a transaction**: partial failure is a first-class outcome (the receipt
  records it), not an abort-and-rollback — rollback is foreclosed-nothing, promised-nothing.
- **Vocabulary creep**: `entities` stays a vocabulary — no relations beyond `parent`, no
  constraints, no codegen. The moment it needs behavior, that is a NEW scope, not a field.
- **Two "pack" meanings in one repo**: mitigated by naming this surface `pack.*` verbs +
  domain-pack language in docs, and never touching `tools/pack`; if confusion bites, the
  extension packager is the one that renames (it is a tool, not a wire surface).

## Open questions

None — the format and semantics shipped and were live-verified downstream; the remaining
choices (bundle encoding, size cap value, lib crate name) are implementation-detail
decisions the PR makes and documents.

## Related

- NubeIO/rubix-ai#13 — the downstream origin (scope `docs/scope/packs/domain-packs-scope.md`
  there): the proven applier this ports, and the embedder that migrates first.
- NubeIO/rubix-ai → `docs/scope/packs/pack-workspace-seed-scope.md` — the **consumer scope** for
  the workspace-seed family (sidebar → nav → access). It owns the pack content + the Packs-UI
  render of the new kinds; this scope owns the `Kind` variants + `apply_*` arms. `Kind::Sidebar`
  (U-pack-sidebar) is shipped; `Kind::Nav` / `Kind::Team` / `Kind::User` (U-pack-nav /
  U-pack-access) are the remaining upstream asks named there.
- NubeIO/rubix-ai#14 — the map widget: the worked example of a widget configured FROM a
  pack's vocabulary (the `geo:` entity hint) — a consumer of `pack.get`.
- `docs/scope/extensions/pack-toolchain-publish-scope.md` — the *extension artifact*
  packager (`lb-pack`), unrelated surface, named here only to kill the collision.
- `docs/scope/store/` — the receipt's internal persistence rides the store crate, not the
  public `store.*` verbs.
