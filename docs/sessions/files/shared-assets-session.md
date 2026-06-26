# Shared workspace assets slice (session)

- Date: 2026-06-26
- Scope: ../../scope/files/files-scope.md + ../../scope/skills/skills-scope.md
  (+ extensions, tenancy, store, auth-caps, mcp)
- Stage: S4 — shared workspace assets (STAGES.md)
- Status: shipped

## Goal

Build S4 as a **vertical slice** through every layer (store → caps → bus → MCP → UI), not "finish
the assets crate": docs/files as workspace assets, skills as versioned grant-gated assets,
extension install records (the persisted `requested ∩ approved`), and team/channel sharing — all
behind capability-checked, membership-checked reads.

**Exit gate (S4), restated as the acceptance criterion:** a doc private to a user can be shared to
a team and linked into a channel; a non-member is DENIED; a skill loads only when granted.

## What changed

### Scopes authored first (the files/skills scopes were stubs)

Per HOW-TO-CODE §1 + SCOPE-WRITTING, the one-line `files-scope.md` TODO and the placeholder
`skills-scope.md` were written into full scopes before any code — the contracts (three-gate model,
content-as-record, the grant relation) come from there.

### store layer — the new `lb-assets` crate (pure state verbs, no auth)

Mirrors `lb-inbox`: the asset models + the raw `lb_store` verbs, workspace-namespaced, **no
authorization** (that's the host's job). One verb per file (FILE-LAYOUT §3):

- **`doc`** — `Doc` (id, owner, title, content, visibility, ts) + `put_doc`/`get_doc`/`list_docs`.
  Content is a **record value**, not a SurrealDB `DEFINE BUCKET` — buckets are not in our embedded
  `kv-mem` build (verified; see debugging entry). The verb shape is bucket-compatible so the S7
  swap to an S3/GCS bucket is config behind the same verb.
- **`skill`** — `Skill` (id, version, author, description, body, ts) + `put_skill` (immutable per
  `{id}@{version}`) / `get_skill` / `list_skills`. Versioned: `skill:{id}@{version}`, rollback =
  loading a prior version's record.
- **`relation`** — the generic `(kind, a, b)` edge backing **all four** sharing facts
  (`share` doc→team, `link` doc→channel, `grant` skill→ws, `member` team→user):
  `relate`/`related`/`unrelate` (revoke = tombstone, since the store has no delete) / `list_related`
  (via a denormalized `pair = {kind}__{a}` filter, the store has no compound-key query).
- **`install`** — `Install` (ext_id, version, granted, ts) + `record_install`/`read_install`.

### caps — **no grammar change needed**

`store:doc/*:read|write` and `store:skill/*:read` already parse and match under the existing
auth-caps grammar (`Surface::Store` + `/`-segmented resource + `*`). The "caps project onto store"
promise paid off: gate 2 is the same `caps::check` chokepoint. Gate 3 (membership/grant) lives in
the host asset service, **not** in caps — a capability says "may use the surface", membership says
"may see *this* asset".

### host — the `assets` service (the three-gate chokepoint), mirroring `channel/`

Every verb runs the gates FIRST, in order, before touching the store:

1. **gate 1 workspace + gate 2 capability** — `authorize_doc`/`authorize_skill` via `caps::check`;
2. **gate 3 membership/grant** — `visibility::may_read_doc` (owner / shared-team-member /
   linked-channel-`sub`-grantee, **reusing the channel capability gate** for the link path) for
   docs; the `grant:skill/{id}` relation for skills.

Verbs (one per file): `put_doc`, `get_doc` (3-gate), `list_docs`, `share_doc` + `link_doc`
(owner-only), `put_skill`, `grant_skill`/`revoke_skill`, `load_skill` (cap + grant), `add_member`.

- **install flow** — `install_extension(node, ws, manifest, wasm, admin_approved, ts)` computes
  `granted = requested ∩ admin_approved`, **persists it** as an `Install` record (persist-before-
  load, mirroring channel's persist-before-publish), then loads. `installed(node, ws, ext_id)`
  reads it back. This closes the S1 deferral ("admin_approved passed in by the caller") —
  `load_extension` keeps its signature (S1–S3 callers unchanged); `install_extension` is the new
  durable verb.

### MCP — the asset verbs over the one contract

`assets.*` is reachable through the MCP contract like any tool: a host-native bridge
`call_asset_tool` runs the **MCP authorize gate** (`mcp:assets.<verb>:call`, workspace-first) then
delegates to the asset verb (which adds its own store + membership/grant gate). Two independent
surfaces, both enforced — an MCP grant never bypasses the store check. `lb_mcp::authorize_tool` is
exposed for host-native tools (avoids the `mcp → host` dependency cycle a `Target::Host` registry
variant would create).

### UI — minimal docs view + api client (mirrors the verbs)

`lib/assets/{assets.types,assets.api}.ts` (one call per export, mirroring the Rust verbs +
node command names) → `lib/ipc/assets.fake.ts` (a faithful in-memory node, including the
membership gate, so the UI's allow/deny paths are exercised in tests) → `features/docs/`
(`useDoc` hook + `DocView` component + barrel). The fake is wired into the existing `fake.ts`
dispatcher (asset commands first, channel fallback). No change to the channel surface.

## Decisions & alternatives

- **Content-as-record, not `DEFINE BUCKET`** — buckets fail to parse on our `kv-mem`,
  `default-features=false` build (empirically verified, debugging entry). Storing content as a
  record keeps "SurrealDB only, no blob service" and workspace-namespace isolation; the verb is
  content-opaque so a real bucket backend is an S7 config swap. **Rejected:** enabling the
  experimental file-storage features now (heavier deps + experimental surface for what is small
  text at S4).
- **Sharing is a relation, not a content copy** — `share`/`link`/`grant`/`member` are one generic
  `(kind, a, b)` edge; a read re-resolves them live, so revoke = delete one edge and the asset
  instantly stops being visible. **Rejected:** minting a per-(user,doc) capability at share time —
  that puts membership in the token (the same anti-pattern auth-caps rejected for the workspace
  claim); a revocable relation beats a grant you must chase down.
- **Records, not SurrealDB `RELATE` edges, at S4** — the membership decision is a few point
  lookups the proven `read`/`list` primitives express directly and testably, without adding
  graph-query surface before the membership graph has a second consumer. Names chosen so a later
  `RELATE`-backed projection is a drop-in (open question).
- **Gate 3 is a SEPARATE layer below caps** — the second isolation layer `tenancy-scope.md`
  deferred ("when the membership graph lands"). A member of team X cannot read a doc shared only to
  team Y, in the same workspace. Ordering is exact: ws → cap → membership; a membership check that
  ran before the workspace check would be a leak (tested by the isolation case).
- **MCP bridge via `authorize_tool`, not a `Target::Host` registry variant** — the asset verbs
  live in `host`, which depends on `mcp`; a `Target::Host` would need `mcp` to call host verbs
  (a cycle). Exposing the MCP authorize gate and bridging in the host keeps the one contract
  without the cycle, and mirrors how `channel` verbs are host-side chokepoints.
- **`install_extension` is a new verb, `load_extension` unchanged** — persisting the grant set
  needs `ws` + `ts`; rather than churn every S1–S3 caller, `load_extension` stays the low-level
  runtime load and `install_extension` is the durable install that records then loads.

## Tests

Mandatory categories that apply at S4 (testing §2): **capability-deny** and **workspace-isolation**
on both the store and the MCP surface — the S4 gate, not extras. Determinism held: all `ts`
injected; a unique workspace id per test; multi-thread flavor on every Node-booting test
(carry-forward from S3 — node-booting tests boot a Zenoh peer).

New this slice:

- **`assets` crate (8)** — `store_isolation_test` (4: doc/skill/relation/install each invisible
  cross-ws — the store-layer wall), `relation_test` (2: idempotent relate/list + revoke-via-
  tombstone), `skill_version_test` (2: immutable version + coexisting versions + rollback).
- **`host/assets_doc_test` (6)** — owner reads private; **shared→team-member reads, non-member
  DENIED** (the exit gate + mandatory membership deny); **linked→channel-`sub`-grantee reads**;
  no read-cap → denied (gate 2 independent of ownership); non-owner cannot share someone else's doc.
- **`host/assets_skill_test` (3)** — **skill loads only when granted** (deny before grant, load
  after, deny after revoke — the §6.12 gate); cap-but-no-grant still denied; latest-granted load +
  pinned rollback.
- **`host/assets_isolation_test` (3)** — ws-B cannot read/list a ws-A doc, nor load a ws-A skill;
  a cross-ws call is refused at gate 1 (mandatory isolation, store surface).
- **`host/assets_mcp_test` (4)** — put→get over the MCP bridge; **MCP deny** (no `mcp:assets.*:call`);
  **store deny through MCP** (mcp cap but no store cap); **MCP isolation** (ws-B can't reach ws-A)
  — the store+MCP isolation the prompt requires.
- **`host/install_record_test` (2)** — install persists `requested ∩ approved` (write not approved
  → absent); install record is workspace-isolated.
- **`ui DocView.test.tsx` (3, Vitest)** — a team member sees the shared doc; a **non-member is
  denied** (the gate surfaced to the user); the owner always sees their own — driving the real
  `assets.api` → `invoke` → fake path.

### Green output

Run per-binary / bounded parallelism — node-booting tests make a single `cargo test --workspace`
OOM (debugging/bus/cargo-test-workspace-ooms-with-many-peers.md).

```
# Rust — light crates (real embedded SurrealDB)
$ cargo test -p lb-assets                              → 8 passed   # NEW crate: store iso + relation + versioning
  auth 4   caps 18   inbox 4   bus 2   ext-loader 2   store 5   mcp 0(unit)   → 35 (S1–S3, unchanged)
  light total: 43 passed   (was 35 at S3; +8 assets)

# Rust — host integration (real wasm + real SurrealDB + Zenoh)
$ cargo test -p lb-host --test spine_test              → 4 passed   # S1 gate, green post-refactor
$ cargo test -p lb-host --test messaging_test          → 3 passed
$ cargo test -p lb-host --test messaging_deny_test     → 3 passed
$ cargo test -p lb-host --test messaging_isolation_test→ 2 passed
$ cargo test -p lb-host --test presence_test           → 2 passed
$ cargo test -p lb-host --test hot_reload_test         → 2 passed
$ cargo test -p lb-host --test cross_node_routing_test → 3 passed
$ cargo test -p lb-host --test offline_sync_test       → 3 passed
$ cargo test -p lb-host --test assets_doc_test         → 6 passed   # NEW: share/link/non-member-deny (EXIT GATE)
$ cargo test -p lb-host --test assets_skill_test       → 3 passed   # NEW: load-only-when-granted (EXIT GATE)
$ cargo test -p lb-host --test assets_isolation_test   → 3 passed   # NEW: MANDATORY ws-isolation (store)
$ cargo test -p lb-host --test assets_mcp_test         → 4 passed   # NEW: MCP deny + isolation (store+MCP)
$ cargo test -p lb-host --test install_record_test     → 2 passed   # NEW: persisted requested∩approved + iso
   host total: 40 passed   (was 22 at S3; +18 assets/install)

   RUST TOTAL: 83 passed, 0 failed   (was 61 at S3; +8 assets +18 host = +22... incl mcp unit unchanged)

# Tauri shell command layer (headless) — unchanged, still green post host-refactor
$ cd ui/src-tauri && cargo test                        → 2 passed

# UI (Vitest) + type-check + bundle
$ cd ui && pnpm test                                   → 11 passed (4 files)   # +3: DocView sharing gate
  ChannelView 3   channel.api 3   useChannel 2   DocView 3
$ pnpm build                                           → tsc --noEmit clean; vite build ✓

# Formatting + file size
$ cargo fmt --all --check                              → FMT OK
$ bash rust/scripts/check-file-size.sh                 → all source files within 400 lines
```

## Debugging

One non-trivial discovery this session, with an entry:

- [store/define-bucket-unavailable-in-kv-mem-build](../../debugging/store/define-bucket-unavailable-in-kv-mem-build.md)
  — `DEFINE BUCKET` fails to **parse** on our `kv-mem`, `default-features=false` SurrealDB build.
  README §6.12 names buckets as the file backing, but they aren't in this build (and §6.12 itself
  flags file support as "experimental"). Fixed by storing content **as a record value** behind a
  bucket-compatible verb — same one-datastore + isolation guarantees; the real bucket backend is an
  S7 config swap. The assets crate's `store_isolation_test` is the regression that proves the
  record path is workspace-isolated.

## Public / scope updates

- Promoted to `public/`: `files` (new — docs as assets, three-gate read), `skills` (new — versioned
  grant-gated), refreshed `public/SCOPE.md` with the S4 row.
- Resolved/refreshed open questions: `extensions` (the "where the admin-approval set is stored"
  open question is now answered — `install:{ext_id}` records via `install_extension`), `tenancy`
  (the deferred membership graph landed as gate 3 — a member of team X can't read team Y's doc),
  `files`/`skills` (the bucket cutover, `RELATE`-vs-records, team-scoped skills, and "shared with
  me" listing remain open).

## Follow-ups

- **Gateway/Tauri command wiring for `assets_*`** — the host verbs + MCP bridge + UI fake exist;
  the SSE/HTTP gateway and Tauri shell don't yet route `assets_*` to `call_asset_tool` (the browser
  reaches the fake in tests). Wire them when the docs UI ships against a real node (mirrors the S3
  channel transport swap).
- **`RELATE`-backed membership** when a second consumer (channel ACLs, tags) appears.
- **Team-scoped skills** and a **"shared with me" doc listing** (S5 agents are the first caller).
- **`DEFINE BUCKET` + S3/GCS backend** behind the same doc verbs when heavy/binary blobs arrive (S7).
- **Sync** of doc/skill/install records — they are `(table,id)` upserts the channel sync path
  already covers; wiring `sync_channel`-style replay for the asset tables is a small follow-up.
- STATUS.md updated? **Yes** — Assets slice marked `shipped`; S4 exit gate met.
