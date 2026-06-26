# Extensions scope — the `github-bridge` as an installed wasm artifact

Status: scope (the ask). Promotes to `public/extensions/extensions.md` once shipped.

Package the **github-bridge** — the inbound edge of the S6 coding workflow — as an installable,
signed **Tier-1 wasm extension**, resolving the explicit S6 deferral (coding-workflow scope §Open
questions: *"Packaging as wasm extensions — when coding-workflow/github-bridge move from host
services to installed artifacts (S7 registry)"*). At S6 the bridge is the host's `ingest_issue`
verb fed by a thin in-test source; this slice moves the **normalization** of a raw GitHub webhook
into a sandboxed wasm component installed through the (now real-HTTP) registry — while the
must-deliver `ingest_issue` write stays a host verb, because the host owns the store/caps seam.

## Goals

- A real `github-bridge` wasm component, installed through `install_from_registry` (signed · verified
  · cached · workspace-isolated), that **normalizes** a raw GitHub webhook JSON payload into the
  canonical `{ issue_id, payload, ts }` triple the host's `ingest_issue` expects.
- Prove the full installed-artifact lifecycle on a *second, non-trivial* extension (the first was
  `hello`): **deny** (no `mcp:registry.install:call` → not installed), **isolation** (ws-B can't see
  ws-A's install/cache), **offline** (install from cache with the registry server down), **rollback**
  (install a prior version with no durable state lost).
- Keep the stable WIT ABI **unchanged** — the bridge uses only the existing `tool.call(name,
  input-json) -> json` export and the `host.log` import.

## Non-goals

- **The orchestrator as wasm.** `triage`/`request_approval`/`resolve_approval`/`start_coding_job`/
  the outbox relay stay **host services** — they drive host-internal seams (`caps::check`, the S5
  agent loop / `ModelAccess`, durable jobs, the transactional outbox) that a sandboxed guest reaches
  only *through* MCP, never *is*. This is the S6 design decision, re-confirmed, not reversed.
- **A guest→host MCP callback.** The wasm bridge does **not** call `workflow.ingest_issue` itself —
  the stable WIT world (`rust/sdk/wit/world.wit`) imports only `host.log`; there is no host-tool-call
  import (the native-tier follow-up "child→host MCP callback transport" is also unbuilt). Adding one
  is a major-bump-class change to the forever ABI (README §11.2) and is explicitly **out of scope**.
  The host calls `ingest_issue` with the guest's normalized output (the "pure-transform bridge").
- A real GitHub webhook **server** / signature (`X-Hub-Signature`) verification / a live HTTP client.
  The bridge transforms a payload it is *handed*; who receives the webhook over HTTP is a later slice
  (it belongs beside `lb-role-registry-host` as a role crate, not in the sandboxed guest).
- New capability grammar, new MCP verbs, new store tables — none. The host verb already exists.

## Intent / approach

**A pure-transform wasm extension behind the existing tool ABI.** The bridge is a stateless guest
exposing one tool, `github-bridge.normalize`, over the same `tool.call` export `hello` uses. Input:
the raw GitHub webhook JSON (issue opened / commented). Output: the canonical
`{ issue_id, payload, ts }` JSON the host's `ingest_issue` already consumes. The guest does only
**deterministic field-mapping** — no IO, no store, no bus, no callback — which is exactly what makes
it a clean Tier-1 sandbox candidate (capability-sandboxed: it gets nothing the host doesn't grant via
WIT imports, and it imports only `log`).

The host side is a **thin composition** of two things that already exist: call the installed
`github-bridge.normalize` tool (via `lb_mcp::call`, the one contract), then pass its output to
`workflow::ingest_issue` (the must-deliver inbox write). One small host helper —
`ingest_via_bridge` — wires guest-output → host-verb; it adds no new seam, it *uses* two.

**Why split the work this way.** The split falls exactly on the trust/seam boundary:
- *Normalization* is pure, portable, untrusted-input-shaped → the sandbox is the right home, and
  packaging it as a signed artifact is the whole point of the registry (a third party can ship a
  bridge for a different forge — GitLab, Gitea — as a drop-in artifact without touching the host).
- *The inbox write* is must-deliver state mutation under a capability gate → the host owns it
  (stateless-extensions §3.4: the durable fact lives in the store via the host, never in the guest).

**Rejected — guest calls `ingest_issue` itself.** Requires a `host.call_tool` import on the forever
WIT ABI (README §11.2, a major bump) plus a host-side capability re-check for guest-originated calls.
Large, boundary-touching, and unnecessary for this slice's value (an installable normalizer). Flagged
to the user; deferred as its own scope. **Rejected — keep the bridge a host service.** Then the S6
deferral is never resolved and the registry never proves a second, real extension end to end.

## How it fits the core

- **Tenancy / isolation:** the install record + cached artifact are workspace-namespaced (the S4
  `Install` + the registry cache, both `(ws, ext_id)`-keyed). ws-B installing/normalizing never sees
  ws-A's install or cache. Tested across **store + MCP**.
- **Capabilities:** install is gated `mcp:registry.install:call` (the deny path: no grant → not
  installed, nothing cached — host-side, transport-independent). The normalize tool is gated
  `mcp:github-bridge.normalize:call` like any tool. The manifest **requests** nothing extra
  (normalization needs no host capability beyond being callable) — the request/grant split shown
  empty, exactly like `hello`.
- **Placement:** `either` — pure transform runs identically on edge or cloud (portable `.wasm`, no
  `if cloud`).
- **MCP surface:** **consumes** the existing host `workflow.ingest_issue` (unchanged); **exposes** one
  new tool `github-bridge.normalize` (a guest tool, gated `mcp:github-bridge.normalize:call`). No new
  host verb, no new grammar.
- **Data (SurrealDB):** none new. The install record (`install:{ext_id}`) and the inbox `Item` (in
  the `triage` channel) are the existing S4/S6 records. State only on the host side; the guest holds
  nothing.
- **Bus (Zenoh):** none — normalization is request/response; the downstream `triage` motion is
  unchanged S6 behavior.
- **Sync / authority:** offline is the registry's cached-install path (install from cache with the
  server down) — proven for this artifact, same guarantee as `hello`.
- **Secrets:** none. Webhook-signature secrets belong to the (out-of-scope) webhook *receiver*, not
  the transform.

## Example flow

1. **Publish.** The `github-bridge` `.wasm` + its `extension.toml` are signed (publisher key, offline)
   into an `Artifact` and placed on the registry origin (the `lb-role-registry-host` server, or the
   in-memory `Source` in tests).
2. **Install.** An admin in ws-A, holding `mcp:registry.install:call`, installs `github-bridge@0.1.0`
   via `install_from_registry` → pull · **verify** (Ed25519 over the digest binding manifest+wasm) ·
   cache · persist `Install` (`granted = requested ∩ admin_approved`). A tampered/foreign-key/unsigned
   artifact is rejected **before caching**, even with the grant.
3. **Webhook arrives** (handed to the host by the out-of-scope receiver) as raw GitHub JSON.
4. **Normalize.** The host calls `github-bridge.normalize` (`lb_mcp::call`) with the raw JSON; the
   sandboxed guest maps it to `{ issue_id, payload, ts }` and returns it. No store, no bus, no
   callback — pure transform.
5. **Ingest.** The host hands the normalized triple to `workflow::ingest_issue` → one idempotent
   inbox `Item` in `triage`, tagged `source:github needs:triage` (the existing S6 contract). The S6
   flow (triage → approval → job → outbox) proceeds unchanged.
6. **Offline / rollback.** With the artifact cached, step 2 re-installs with the server down (offline);
   installing `0.1.0` after `0.2.0` is rollback (registry semantics) — the inbox `Item`s written
   before the swap are intact after (no durable state in the guest).

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md` §2), exercised on this **second** real
extension. Pattern-match the `hello` idioms in `crates/host/tests/registry_*.rs` (publisher/sign/
principal/`MapSource` fixtures) and `http_source_test.rs`:

- **Capability-deny (mandatory):** `install_from_registry` for `github-bridge` is refused without
  `mcp:registry.install:call` (asserted at `authorize_registry`, before any fetch/cache); and
  `github-bridge.normalize` is refused without `mcp:github-bridge.normalize:call`.
- **Workspace-isolation (mandatory):** ws-B cannot see ws-A's `github-bridge` install record or cached
  artifact; a normalize call in ws-B can't reach ws-A's installed tool. Across **store + MCP**.
- **Offline (mandatory here — installed artifact):** once cached, `github-bridge@0.1.0` installs again
  with the registry `Source` offline (the cached path never fetches).
- **Rollback / hot-reload (mandatory here):** install `0.2.0` then re-install `0.1.0`; an inbox `Item`
  ingested before the swap survives (no durable guest state lost).
- **Happy / round-trip (integration):** a signed `github-bridge` installs through the real registry,
  and a real GitHub webhook payload run through `github-bridge.normalize` → `ingest_issue` lands the
  expected `triage` inbox `Item` (`source:github needs:triage`, id = issue id, idempotent on retry).
- **Unit (guest, pure):** the normalize mapping itself — issue-opened and issue-comment payloads map
  to the right triple; a malformed payload returns `bad-input` (the WIT `tool-error`), not a panic.

Build commands for the new guest (carry into the session doc):
`(cd rust/extensions/github-bridge && cargo build --target wasm32-wasip2 --release)`.

## Risks & hard problems

- **Resisting the ABI-widening temptation.** The "obvious" design (guest calls `ingest_issue`)
  silently grows the forever WIT world. The discipline is to keep the guest a pure transform and let
  the host compose — the risk is a future contributor adding `host.call_tool` casually. The scope
  names it out-of-scope precisely to make that a conscious decision, not a drift.
- **Idempotency on retry** must stay on the host's `ingest_issue` (`(channel, id)` upsert), unchanged
  — the transform is stateless so a re-delivered webhook must still produce one item. Tested.
- **Payload shape coupling.** The normalize mapping encodes GitHub's webhook JSON shape; a GitHub API
  change breaks it. That's *contained in the guest* (re-publish a new artifact) — which is the
  argument for packaging it as a swappable extension in the first place.

## Open questions

- ~~**Where the host calls `normalize` from.**~~ **RESOLVED:** shipped as `lb_host::ingest_via_bridge`,
  a typed host helper (test/UI drives it), mirroring how S6 exposes `start_coding_job`. A
  webhook-receiver role crate that drives it on a real HTTP POST (with `X-Hub-Signature` verification)
  is the follow-up — belongs beside `lb-role-registry-host`, not in the sandboxed guest. **NOW
  SHIPPED:** `lb-role-github-webhook` (see [`github-webhook-scope.md`](github-webhook-scope.md)).
- ~~**Does `github-bridge.normalize` need a host capability at all?**~~ **RESOLVED:** no — it is a pure
  transform (`request = []`), gated only by its own `mcp:github-bridge.normalize:call`. A future bridge
  variant needing e.g. a redaction secret would add a request line; not now.
- ~~**Manifest `visibility = "public"`.**~~ **DECIDED:** ships `private` (one workspace). The
  public-catalog union (one signed bridge installable cross-workspace) stays a registry follow-up.
- **Multi-forge.** GitLab/Gitea bridges as sibling artifacts sharing the `{issue_id, payload, ts}`
  output contract — the contract is what this slice fixes; the siblings are future artifacts. (Open.)
- **Guest→host MCP callback (`host.call_tool`).** If a future guest genuinely needs to invoke a host
  tool from inside the sandbox, that is a deliberate addition to the forever WIT world (README §11.2) —
  its own scope, explicitly out of this slice. (Open.)

## Status

**Shipped (2026-06-26).** See `../../sessions/extensions/github-bridge-session.md` and
`../../public/extensions/extensions.md`. Debugging:
`../../debugging/extensions/loaded-extension-instance-is-node-global.md`.

## Related

- `coding-workflow/coding-workflow-scope.md` (the S6 flow + the deferral this resolves — §Open
  questions "Packaging as wasm extensions"), `extensions/extensions-scope.md` (the manifest contract),
  `registry/registry-scope.md` (the signed install path), `auth-caps/auth-caps-scope.md` (the grant
  grammar the manifest requests).
- `rust/sdk/wit/world.wit` (the stable ABI this slice does **not** touch), `rust/extensions/hello/`
  (the reference Tier-1 extension this mirrors).
- README `§6.3` (runtime tiers), `§6.4` (registry/distribution), `§6.5` (MCP), `§6.16` (workflow
  extensions), `§11.2` (the WIT forever boundary), `§13` (the manifest contract).
