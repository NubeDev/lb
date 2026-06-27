# Extensions scope — lifecycle management (start · stop · enable · disable · upload · install · delete) over the gateway

Status: scope (the ask). Promotes to `public/extensions/` once shipped. A follow-up to
`extensions-scope.md` (the runtime + two tiers), `native-tier-scope.md` (the Tier-2 supervisor), and
`registry/registry-scope.md` (signed pull/verify/cache/install). Those slices built the *mechanisms*;
this slice makes them a **complete, manageable lifecycle reachable from a browser**, not just the
desktop Tauri shell.

Today an extension's lifecycle is **half-built and desktop-only**. The host has `install_extension`
(Tier-1 wasm), `install_native`/`stop_native`/`restart_native`/`status_native` (Tier-2), and
`install_from_registry`/`registry.list`/`registry.resolve` (registry) — but **none are wired to the
gateway** (only the Tauri command layer reaches them, so a browser throws `unknown command`), and the
set has **holes**: there is no `uninstall`/`delete` for either tier, no `enable`/`disable` as a durable
intent, no way to **list installed extensions** in a workspace, no `start` for a wasm extension, and no
**upload/publish** path to add an artifact to the registry. This slice closes the matrix and exposes it
over both transports so extensions can actually be **managed for real** from the UI.

## Goals

- **A complete, uniform lifecycle verb set** across both tiers, each capability-gated and workspace-scoped:
  `install` · `start` · `stop` · `enable` · `disable` · `restart` · `uninstall`/`delete` · `status` ·
  `list` (installed). Where a verb is a no-op for a tier (a wasm component has no separate OS process to
  `start`), it resolves to the tier's equivalent intent (load/unload) rather than being absent — the
  caller sees one consistent surface.
- **`enable`/`disable` as a durable intent, distinct from `start`/`stop`.** `disable` means "do not run
  this, and do not auto-start it on boot"; `stop` means "stop the running instance now" (it may be
  re-started). A boot reconciler honors `enabled ∧ started` (the native-tier follow-up, now owned here).
- **`uninstall`/`delete` for both tiers** — stop/unload the instance, remove the runtime registration,
  and **delete the durable `Install` record** (+ native binary, + `native_status`), workspace-scoped.
  Idempotent: deleting an absent extension is a success, never a leak across workspaces.
- **`list` installed extensions per workspace** — enumerate `Install` records with current lifecycle +
  health, so the UI can render a real extension table. This is the verb that is conspicuously missing.
- **An `upload`/`publish` path to the registry** — a workspace admin uploads a **signed** `extension.toml`
  + wasm/native artifact (or a packaged bundle) to the registry-host, which verifies the signature against
  the publisher allow-list and stores it as a catalog entry. Install then proceeds through the existing
  signed pull. Upload is the **producer** side the registry never had.
- **Expose the whole set over the gateway HTTP routes** (`registry_*`, `native_*`, `ext_*`), mirroring
  each host verb 1:1, so the **browser** can manage extensions exactly as the Tauri shell does — the same
  four-file move the channel/collaboration surfaces proved.
- **An extension-management UI** — a real `features/extensions/` console (list installed · install from
  catalog · start/stop/enable/disable · uninstall · upload) that supersedes the demo-grade `RegistryView`
  / `NativeView`, driven over the real routes with a fake that matches them 1:1 for tests.

## Non-goals

- **No new tier and no SDK/WIT change.** Tier-1 wasm + Tier-2 native are the tiers (README §6.3); the
  WIT boundary is unchanged. If a verb appears to need a guest→host callback, that's the deferred
  `host.call_tool` ABI question, out of this scope.
- **No registry federation / public marketplace.** Upload targets the workspace's own registry-host with
  its publisher allow-list. A public, multi-publisher catalog with trust delegation is a later registry
  slice (`registry-scope.md` follow-ups).
- **No grant/role administration.** *Who* may call these verbs is the `auth-caps/admin-crud-scope.md` +
  `authz-grants-scope.md` model; this scope defines the verbs and their **own** capability gates, not the
  admin surface that assigns them.
- **No OS-level hardening in this slice** (cgroups/seccomp/userns) — still a native-tier follow-up. This
  slice is the lifecycle *surface*, not the sandbox.
- **No module-federated extension *pages*.** Mounting an extension's own UI is the deferred
  `scope/extensions/` UI-federation scope. Here the UI manages extensions; it does not host their pages.
- **No hot multi-version running.** `enable`/`install` of a new version replaces the prior (rollback =
  install prior version, as today). Side-by-side versions are out.

## Intent / approach

**One lifecycle trait, two tier backends, one gateway mirror.** The verbs already differ per tier in
ad-hoc functions; this slice unifies them behind a small host **lifecycle surface** that dispatches to
the wasm runtime or the native supervisor by the `Install` record's `tier`. The durable truth stays
where it is — the `Install` record (+ `native_status`) — and the live truth stays in the runtime maps
(the wasm component registry, the native `SidecarMap`). The new verbs (`disable`, `uninstall`, `list`)
are records-plus-runtime operations in exactly that existing split; **no new persistence layer**.

```
                 gateway routes (browser)        Tauri commands (desktop)
                        │  ext_* / native_* / registry_*   │
                        └───────────────┬───────────────────┘
                                        ▼
                       host lifecycle surface  (dispatch by Install.tier)
                        ┌──────────────┴───────────────┐
                   Tier-1 wasm runtime           Tier-2 native supervisor
                   (component registry)          (SidecarMap + Launcher)
                                        │
                    durable truth: Install record (+ native_status), workspace-scoped
```

- **`enable`/`disable`** add a durable `enabled: bool` (or a `Lifecycle` enum gaining `Disabled`) to the
  `Install` record. `disable` implies a `stop`/unload; `enable` makes it eligible to run and (per intent)
  starts it. A **boot reconciler** re-applies `enabled ∧ started` on node start — the native-tier
  follow-up, now built here for both tiers.
- **`uninstall`** = stop/unload + delete the runtime registration + delete the `Install` (+ `native_status`,
  + cached binary). Workspace-scoped delete; idempotent; ws-isolation enforced before the record is touched.
- **`list`** = query `Install` records for the workspace, join the live runtime/health state, return a row
  per extension `{ext, version, tier, enabled, running, health, restart_count}`.
- **`upload`/`publish`** = a new registry-host endpoint (`POST /artifacts/…`) that takes a **signed**
  artifact, runs `verify_artifact` against the publisher allow-list **before** storing it as a catalog
  entry. The host gains a `registry.publish` verb that drives it (an outbox `Target` write for durability,
  matching the registry-scope follow-up). Install is the existing verified pull — unchanged.
- **Gateway mirror**: each verb gets a route in `role/gateway` that reads the session token, verifies it,
  and calls the host verb with the token's principal — identical to the collaboration slice's pattern.
  The `ui/src/lib/ipc/http.ts` command→route map gains the `ext_*`/`native_*`/`registry_*` entries it is
  missing today.

**Rejected alternatives:**
- *Leave lifecycle on Tauri only.* Rejected — the browser is the primary demo transport (collaboration
  slice); an admin who cannot manage extensions from a browser cannot manage the platform. The verbs must
  be transport-agnostic host verbs with a thin gateway mirror, not desktop commands.
- *Model `disable` as just `stop`.* Rejected — they have different boot semantics. A stopped extension
  re-starts on boot; a disabled one must not. Conflating them means a "disabled" extension silently comes
  back after a restart — a real operator surprise. Durable intent is the point.
- *Hard-delete with no idempotency.* Rejected — uninstall must be safe to retry (relay/UI re-issue) and
  must never delete across the workspace wall. Idempotent, workspace-first delete.
- *A separate `lb-lifecycle` crate.* Rejected for now — the verbs are thin host services over the existing
  runtime + supervisor + registry seams; a new crate adds a boundary without new behavior. Keep them as
  host services next to `install`/`native`/`registry`. (Revisit if the surface grows.)

## How it fits the core

- **Tenancy / isolation:** every verb is workspace-first — the `Install` record, the runtime map key, the
  native `SidecarMap` key, and the registry catalog entry are all `(ws, ext)`-scoped. A ws-B caller can
  never list, start, stop, uninstall, or upload into ws-A. `uninstall` resolves the workspace **before**
  touching any record. The mandatory two-workspace isolation test runs over store + MCP + the runtime maps
  (extending the native-tier isolation test to the new verbs).
- **Capabilities:** each verb is gated `mcp:<surface>.<verb>:call` — `mcp:ext.uninstall:call`,
  `mcp:ext.disable:call`, `mcp:ext.list:call`, `mcp:registry.publish:call`, reusing the existing
  `mcp:native.*` / `mcp:registry.*` gates for the verbs that have them. Deny is opaque. Upload additionally
  requires the publisher key to be on the registry-host allow-list — a **second** gate (authenticity)
  before authority, mirroring the webhook ingress pattern.
- **Symmetric nodes:** the verbs are config-free host services; the gateway is a role and the Tauri shell
  runs the node in-process — two transports over one verb set. No `if cloud {…}`. The boot reconciler runs
  on every node identically.
- **One datastore:** lifecycle state is `Install` (+ `native_status`) records and registry catalog entries
  in SurrealDB. No new store. The native binary cache is the existing on-disk cache (S8 persistent path).
- **State vs motion:** lifecycle is **state** (records). A "extension X started/stopped/uninstalled" event,
  if surfaced live in the UI, is ordinary motion the host publishes — not part of the durable verb.
- **Stateless extensions:** unchanged and reinforced — because an instance holds no durable state,
  `stop`/`uninstall`/`disable` are safe; all truth is in the `Install` record and the workspace's data.
  This is exactly why hot-reload and uninstall are clean.
- **MCP is the contract:** the verbs are MCP tools; the gateway, the Tauri shell, an admin agent, and
  another extension all call them identically. The UI is just one caller.
- **Durability:** `upload`/`publish` is a must-deliver effect (the artifact must land in the catalog) —
  it rides the **outbox** `Target` to the registry-host (matching the registry follow-up), not raw pub/sub.
  Uninstall's record delete is a single transactional write.
- **One responsibility per file:** one verb per file under `host/src/{ext,native,registry}/` (e.g.
  `ext/uninstall.rs`, `ext/disable.rs`, `ext/list.rs`, `registry/publish.rs`); one route per file in the
  gateway; `features/extensions/` follows the `features/<x>/` view+hook+api shape.
- **SDK/WIT impact:** **none** — flagged explicitly. Every verb drives host-internal seams (runtime,
  supervisor, registry, records). The only WIT-adjacent question (`host.call_tool`) is out of scope.

## Example flow

1. **Admin opens the extensions console** (browser → gateway). `ext_list` returns the workspace's
   installed extensions with live state: `hello@v2` (wasm, enabled, running), `echo-sidecar@v1` (native,
   enabled, running, restarts=0).
2. Admin **uploads** a signed `hvac@v1` bundle → `registry_publish` → the registry-host `verify_artifact`s
   it against the allow-list and stores the catalog entry. It appears in the catalog list.
3. Admin **installs** `hvac@v1` from the catalog → existing verified pull → `Install` record + runtime
   load; the console row shows `enabled, running`.
4. Admin **disables** `hvac@v1` → durable `disabled`, instance unloaded; the row shows `disabled, stopped`.
   The node is **restarted** → the boot reconciler does **not** start `hvac` (disabled intent honored),
   but **does** start `hello`/`echo` (enabled). 
5. Admin **uninstalls** `echo-sidecar` → the sidecar process is cooperatively stopped, the `Install` +
   `native_status` + cached binary are deleted; the row disappears. Re-issuing uninstall is a no-op success.
6. A **ws-B admin** logs in and `ext_list`s — sees **none** of ws-A's extensions; cannot start, uninstall,
   or upload into ws-A (opaque deny / empty list). The wall holds across the whole lifecycle.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`:

- **Capability deny** — over the **real gateway route** and over MCP: a principal without
  `mcp:ext.uninstall:call` / `ext.disable:call` / `ext.list:call` / `registry.publish:call` is refused;
  the UI surfaces `Denied`. Each new verb has its own deny test.
- **Workspace isolation** — a ws-B session cannot `ext_list`, start, stop, disable, uninstall, or publish
  into ws-A; `uninstall` of a ws-A ext id by a ws-B caller deletes **nothing** and leaks nothing. Across
  **store + MCP + the runtime/SidecarMap**. Extends the native-tier isolation test to the new verbs.
- **Offline / sync** — `Install` lifecycle records (including `disabled`/deleted) replay idempotently after
  an offline edit; a re-issued uninstall after reconnect does not double-delete or resurrect.

Plus this slice's cases:

- **Lifecycle correctness** — `enable→disable→enable` round-trips; `stop` then **boot reconcile** restarts
  (enabled), `disable` then boot reconcile does **not** (the load-bearing distinction); `restart`
  increments `restart_count`; `uninstall` removes the record and the row.
- **Idempotency** — double `uninstall`, double `disable`, double `publish` (same digest) are each a no-op
  success; no duplicate catalog entries; no cross-ws deletion.
- **Upload/verify** — a signed artifact publishes and installs; a **tampered/unsigned/foreign-key** upload
  is **rejected before storing** (authenticity gate), even with `registry.publish` granted.
- **List fidelity** — `ext_list` reflects every state transition (install/disable/uninstall) and joins live
  health (running/restart_count) for native rows.
- **Gateway parity** — each verb tested through the real node over the gateway (mirror `gateway_test`), and
  the `http.ts` map has an entry for each (no `unknown command` in the browser).
- **Vitest** — an `ExtensionsView` test per operation on the fake (mirror existing view tests); the fake
  matches the route contracts 1:1.

## Risks & hard problems

- **`enable`/`disable` vs `start`/`stop` is easy to conflate and load-bearing.** Get the boot semantics
  wrong and a "disabled" extension silently returns after a restart. The durable intent + the reconciler
  honoring it is the whole point — test the restart explicitly (the case above).
- **`uninstall` must be transactional and workspace-first.** Partial deletes (record gone, binary orphaned;
  or instance unloaded but record left) leave the system inconsistent. Resolve the workspace, stop/unload,
  then delete record+status+binary as one logical operation; make it idempotent.
- **The gateway-exposure surface is broad.** Every `native_*`/`registry_*`/`ext_*` verb needs a route + an
  `http.ts` map entry + a fake; drift between fake and route gives green tests against a wrong shape (the
  collaboration slice's lesson). Keep verb names + payloads 1:1; consider a shared contract list.
- **Upload introduces a producer the registry never had.** The publisher allow-list, signature-before-store,
  and durable catalog backing (the outbox `Target`) are new trust surface. An unsigned/foreign artifact
  reaching the catalog is a supply-chain hole — the authenticity gate must precede storage, not follow it.
- **The boot reconciler can fight an in-flight operation.** Reconcile on boot only (not a tight loop), and
  make it idempotent against the live maps so it never double-starts. Coordinate with the native-tier
  health-poll follow-up so they don't both restart.
- **Tier asymmetry leaking into the surface.** A wasm component has no OS process; a sidecar does. The
  unified verb set must resolve each verb sensibly per tier without an `if tier` smell in the caller —
  dispatch lives in the host lifecycle surface, by the record's `tier`, once.

## Open questions

- **`enabled` field vs a `Lifecycle` enum gaining `Disabled`/`Uninstalled`.** Lean: extend the existing
  native `Lifecycle` and add a parallel `enabled` flag to the wasm `Install`, unified by the lifecycle
  surface — but confirm one shape across both tiers so `list` returns a uniform row.
- **Does `uninstall` evict the cached binary, or keep it for fast reinstall?** Lean: evict on uninstall
  (clean delete), keep cache GC separate (a registry follow-up). Reinstall re-pulls from cache-or-origin.
- **Upload packaging format** — a single signed bundle (toml+wasm+sig) vs separate parts. Lean: reuse the
  registry's existing `Artifact` shape (digest binds manifest+wasm) so verify is unchanged; bundle is just
  its transport.
- **Where the boot reconciler lives** — the `node` binary (config-mounted, like the workflow driver) vs a
  host service called on boot. Lean: a host `reconcile` verb the `node` calls on start, so it's testable
  headlessly and symmetric.
- **Should `ext_list` be one verb across tiers or `registry`/`native` specific?** Lean: one `ext.list`
  that unions both tiers from `Install` records, with `tier` on each row — one console table, not two.
- **Multi-version / rollback interaction.** Rollback today = install prior version. Does `disable` of a
  version then `enable` of another count as rollback? Lean: yes — install/enable replaces; no side-by-side.

## Related

- `scope/extensions/extensions-scope.md` — the runtime + two tiers this gives a full lifecycle.
- `scope/extensions/native-tier-scope.md` — the Tier-2 supervisor; this owns its deferred boot-reconciler
  and adds `uninstall`/`disable`.
- `scope/registry/registry-scope.md` — pull/verify/cache/install; this adds the **upload/publish** producer
  and the durable catalog backing it flagged as a follow-up.
- `scope/frontend/admin-console-scope.md` — the admin UI that renders and drives these verbs (the console
  for extensions lives there or in `features/extensions/`, sharing the admin shell).
- `scope/auth-caps/authz-grants-scope.md` + `admin-crud-scope.md` — who may call these verbs (the grants),
  vs the verbs themselves (here).
- `scope/inbox-outbox/outbox-scope.md` — the `Target`/relay the durable `publish` rides.
- README **§6.3** (extension tiers), **§6.4** (registry & distribution), **§6.13** (extension UIs),
  **§11.5** (the install intersection / blast radius).
</content>
</invoke>
