# Extensions scope — the extension SDK: generate · build · publish (in-shell, MCP-driven)

Status: scope (the ask). Promotes to `public/extensions/extensions.md` (dev-flow section) once shipped.

We want a **reusable extension SDK** — one path that takes someone from "nothing" to "installed and
loaded on a running node" without hand-assembling a crate. It does three jobs, all reachable from the
shell over MCP: **generate** a fresh extension (pick `wasm` or `native`, get a working backend + a
shadcn/Tailwind federated UI page), **build** an extension folder via the local toolchain, and **publish**
it through the signed-`Artifact` flow that already exists. The headline use is an in-shell **Extension
Studio** wizard: a user points at `rust/extensions/proof-panel`, builds it, and uploads it — or scaffolds
a new extension, picks the tier and the core features they want, and publishes — all without leaving the UI.

## Goals

- A reusable **`lb-devkit` library crate** that performs scaffold + build, with one signing idiom shared
  with the existing `lb-pack` (no second crypto stack).
- A **`devkit` host service + `devkit.*` MCP verbs** so the shell drives scaffold/build the same way every
  other capability is called (rule 7) — gated, **local-only**, behind one `Toolchain` trait.
- A **generator** that emits a *compiling, publishable* extension folder: backend crate, `extension.toml`
  with the chosen capabilities, `build.sh`, and a co-located **shadcn/ui + Tailwind** federated `ui/` page
  wired to the host bridge — rendered from the shipped `proof-panel`/`fleet-monitor` shape, no placeholders
  (CLAUDE §9).
- A **built-in Extension Studio page** (local-only shell view) that walks: *generate (tier + features) →
  build → publish*, or *select an existing folder → build → publish*, showing the real `204/403/422`
  outcome and the live build log.

## Non-goals

- **No developer CLI in this scope.** The SDK is driven from the shell over MCP. A thin `lb-ext` CLI over
  the same `lb-devkit` library is a natural later addition, but it is **out of scope here** — building it
  now would split effort before the library + service exist. (Noted, not built.)
- **No new wire format or trust model.** Publish reuses the existing `lb-pack` digest+Ed25519 `Artifact`,
  `POST /extensions`, `ext.publish`, and `LB_TRUSTED_PUBKEYS` exactly (dev-flow.md). The SDK is ergonomics
  over that path, not a second one.
- **No remote/cloud build farm.** Building shells out to the local `cargo`/`pnpm` toolchain; the build
  capability is **local-only** (a cloud node does not grant it). Distributed/sandboxed build is a follow-up.
- **No change to the manifest contract** (`extensions-scope.md`) or the capability grammar shape — the
  generator *writes* an `extension.toml`; it does not invent new manifest fields.
- Not the lifecycle verbs (start/stop/enable/disable/delete) — that is `lifecycle-management-scope.md`.
  This scope ends at "published + loaded"; lifecycle takes over from there.
- Not publishing the templates as a separately versioned package — they ship embedded in the crate.

## Intent / approach

**One library, one service, one page.** Factor the real work into a reusable library crate `lb-devkit`
(`rust/crates/devkit/`); expose it as a host service (`crates/host/src/devkit/`) with a `devkit.*` MCP
bridge; drive it from the built-in Studio page. The signing code is **extracted from `lb-pack` into a
shared library function** that both `lb-pack` and the `devkit` publish path call, so there is exactly one
crypto idiom (`lb-pack`'s binary becomes a thin wrapper over the same lib — no behavior change, no
duplicate signing).

- **`devkit.scaffold`** — render a fresh extension folder from a template. Bounded/fast → synchronous.
- **`devkit.build`** — build a folder via the local toolchain. Long-running → a **durable job** (§6.10)
  returning a job id + a log subject; the page streams the log over `bus.watch` SSE.
- **`devkit.templates`** — list available tiers + their feature toggles, so the wizard renders choices
  without hardcoding them.
- **`devkit.inspect <path>`** — read a folder's `extension.toml` (tier, declared tools, caps, built-or-not)
  *and report toolchain readiness*, to drive "select an existing dir" and pre-flight a build.
- **Publish stays `ext.publish`** — the SDK does not fork the publish path. The Studio's "Publish" button
  packs (signs) **server-side** and POSTs through the unchanged gate.

The generator is **template-driven**: one embedded template tree per tier (`wasm`, `native`) under
`rust/crates/devkit/templates/{wasm,native}/`, each file a single responsibility (FILE-LAYOUT), rendered
with the chosen `id`, capabilities, and feature toggles. The `wasm` template is the proof-panel shape and
the `native` template the fleet-monitor/echo-sidecar shape — so a generated extension is *exactly* a
shipped reference extension with the names swapped, the strongest guarantee it builds. The templates pin
the **current** WIT `world` major so a freshly generated extension matches the host
(`extensions-scope.md`: a `world` mismatch is refused at load); they are versioned with the crate and
updated whenever the WIT major bumps.

**The load-bearing decision: build is a gated, local-only host capability.** The user's ask is "a wizard
in the UI that builds and uploads." Building means running `cargo`/`pnpm` against a folder — arbitrary
code execution outside any sandbox. We expose it as a host service, but draw the wall hard: `devkit.build`
requires `mcp:devkit.build:call`, is **local-only** placement (a cloud node never grants `devkit.*`, so
symmetric-nodes holds — config/role, not an `if cloud`), and operates **only on paths under a configured
allow-root** (`LB_DEVKIT_ROOT`, default `rust/extensions/`). The path is canonicalized and checked for
traversal/symlink escape before any process spawns. We rejected keeping build pack-only (too weak for the
stated wizard) and an unrestricted build path (a remote-code-execution hole). The `cargo`/`pnpm`/wasm-target
invocation is the sanctioned "true external behind one trait" carve-out (§9) — it cannot run in-process.

**Studio is a built-in shell view, not itself an extension.** It bootstraps extensions, so it cannot be one
that must be published first (chicken/egg). It is a built-in **local-only** view, always present on a local
node, absent on a cloud node — the same placement rule as the `devkit.*` verbs it calls.

## How it fits the core

- **Tenancy / isolation:** `devkit.*` authorizes workspace-first like every verb; it ends in the existing
  ws-scoped `ext.publish` (artifact cached + `Install` persisted in the caller's ws). Scaffold/build touch
  the **filesystem dev tree**, not workspace records — they produce an artifact that *then* enters a
  workspace through the unchanged publish gate. The build **job record** is ws-scoped.
- **Capabilities:** new verbs `mcp:devkit.scaffold:call`, `mcp:devkit.build:call`, `mcp:devkit.templates:call`,
  `mcp:devkit.inspect:call` (auth-caps grammar, no new shape). Deny path is the headline test: a caller
  without `mcp:devkit.build:call` → `Denied`, nothing runs; a build path **outside the allow-root** →
  refused before any process spawns. Publish keeps its two existing gates (cap `ext.publish` 403 +
  signature 422).
- **Placement:** `devkit.*` and the Studio page are **local-only** (`extensions-scope.md` `placement`). No
  cloud build. This is the one place placement carries real behavior here, and it is config/role — no code
  branch.
- **MCP surface (API shape §6.1):**
  - *Create / action:* `devkit.scaffold` creates a folder tree (synchronous, bounded); `devkit.build` is an
    action verb (produces artifacts) — no update/delete (regenerate or `rm` by hand; the dev tree is not a
    managed resource).
  - *Get / list:* `devkit.templates` (list tiers + feature toggles); `devkit.inspect` (read a folder's
    manifest + toolchain readiness).
  - *Live feed:* **build streams logs** over the existing bus/SSE (`devkit/build/{job_id}` subject →
    `GET /bus/stream`), reusing the generic `bus.publish`/`bus.watch` shipped in S10 — not a blocking call
    that returns only at the end.
  - *Batch / jobs:* `devkit.build` **enqueues a durable job** (§6.10, jobs-scope) and returns a job id; the
    wizard watches the job's status + the log stream. A blocking cargo build in a tool handler is exactly
    the smell §6.1 warns against (no resume, ties up the call). Scaffold is bounded/fast → synchronous.
- **Data (SurrealDB):** the only durable records are the **build job** (jobs plane) and, at the end, the
  unchanged publish records (catalog + `Install`). Templates and the dev folder live on disk — inputs to an
  artifact, not platform state. One datastore holds intact.
- **Bus (Zenoh):** build-log motion is fire-and-forget over `bus.publish` (`devkit/build/{id}` subject). No
  must-deliver effect originates here, so **no outbox** — the durable outcome is the job record + the
  publish, both already durable.
- **Sync / authority:** node-local; the dev tree is on the box running the node. A built artifact syncs only
  once published (the existing registry path).
- **Secrets:** the publisher signing key (`lb-pack`'s Ed25519 seed) is the only secret. The **UI wizard
  never sees it** — the node signs server-side from `LB_DIR/keys`, exactly the custody `lb-secrets` mandates;
  the key is never returned to the page or logged.
- **Stateless extensions:** the template enforces it (generated state goes to `store`/`kv`); the `devkit`
  service itself holds nothing durable beyond the job record.
- **SDK/WIT impact:** **none to the WIT boundary** — the generator emits code that *targets* the existing
  `world`; it does not change it. Flagged because the templates must pin the **current** `world` major (a
  mismatch is refused at load) and are re-pinned on every WIT major bump.

## Example flow (the Studio wizard, generate path, end to end)

1. User opens the **Extension Studio** page (local node). It calls `devkit.templates` → renders "Tier:
   wasm | native" and per-tier feature toggles: `ui` (shadcn page), `series-read` (read a series — the
   proof-panel default), `ingest` (write via `ingest`), `kv` (per-ext record store). Toggles compose
   freely; each adds a tool stanza to `extension.toml` + a matching code stub.
2. User picks `native`, id `cooler-panel`, ticks `ui` + `ingest`, clicks Generate. The page calls
   `devkit.scaffold {id, tier:"native", features:["ui","ingest"]}` (gated `mcp:devkit.scaffold:call`). The
   service renders the `native` template into `LB_DEVKIT_ROOT/cooler-panel/` with the requested caps
   already in `extension.toml`. Returns the path + the file list.
3. User clicks Build. The page calls `devkit.build {path:"…/cooler-panel"}` (gated `mcp:devkit.build:call`;
   path validated under the allow-root). The service **enqueues a build job**, returns
   `{job_id, log_subject:"devkit/build/<job_id>"}`.
4. The page opens `GET /bus/stream?subject=devkit/build/<job_id>` and shows the live `cargo`/`vite` log.
   The job runs `cargo build --release` (native) or `--target wasm32-wasip2` (wasm) + `vite build` for
   `ui/`, behind the `Toolchain` trait, and marks the job `done`/`failed`.
5. User clicks Publish. The node packs (signs server-side), `POST /extensions` semantics → `ext.publish` →
   verify → cache → `install_extension` → loaded live. `204` ⇒ the new extension is callable and its page
   appears in the sidebar with no restart.
6. **Deny path:** the user's token lacks `mcp:devkit.build:call` → step 3 is `Denied`, no process spawns.
   A path outside `LB_DEVKIT_ROOT` → refused before spawn. A cloud node → `devkit.*` not granted at all.

## Implementation decisions recorded during build

- The Studio publish shortcut keeps the existing `POST /extensions` route and `ext.publish` semantics,
  but accepts `{ "path": "<folder-under-LB_DEVKIT_ROOT>" }` in addition to the signed `Artifact` body.
  The gateway resolves the path under `LB_DEVKIT_ROOT`, reads the built output, signs server-side from
  `LB_DIR/keys/dev-publisher.key`, then feeds the resulting `Artifact` into the normal verify/cache/
  install path. This is not a new publish verb and does not expose the key to the page.
- Devkit build logs are JSON string payloads on the generic bus subject. `/bus/stream` already emits
  JSON payloads, so raw text bytes would arrive as `null`.

## Testing plan

Mandatory categories (`scope/testing/testing-scope.md`), all against the **real** store/bus/gateway + the
real toolchain (no mocks; `cargo`/`pnpm` are the sanctioned external behind the `Toolchain` trait, §0):

- **Capability deny (mandatory):** `devkit.build`/`devkit.scaffold` without the grant → `Denied`, nothing
  spawns/writes. A build `path` outside `LB_DEVKIT_ROOT` (incl. a traversal/symlink attempt) → refused
  before any process. Publish keeps its existing 403 (cap) and 422 (untrusted/tampered) tests — assert the
  SDK path hits them unchanged.
- **Workspace isolation (mandatory):** the publish a Studio run produces lands in the caller's workspace
  only; ws-B cannot see ws-A's published catalog entry/`Install` (reuse the registry isolation test). The
  build job record is ws-scoped.
- **Generator correctness (the load-bearing test):** `devkit.scaffold` (wasm) → `devkit.build` → publish
  against a spawned node → `204` and the tool is callable. Same for `native`. This is the proof the
  templates aren't placeholders — a generated extension goes green end to end. Run for both tiers in CI
  (the FILE-LAYOUT size check already guards the template files).
- **Build-as-job / offline:** a build job survives the call returning (the wizard reconnects to the log
  stream after a reload and still sees completion); a killed node mid-build leaves the job `failed`/resumable
  per jobs-scope, not a wedged call.
- **Frontend (real gateway):** the Studio page over the bridge — a `*.gateway.test.tsx` driving
  `devkit.templates` → `scaffold` → `build` (watching the real SSE log) → publish against a spawned node,
  plus a Playwright e2e of the generate→build→publish wizard.

## Risks & hard problems

- **Build is arbitrary code execution.** This is the whole risk, and the mitigations are real, not advisory:
  cap gate + **allow-root path validation** (canonicalize, reject traversal/symlink escape) + local-only
  placement. The test proves a path outside the root is refused on a *real* spawn attempt, not a displayed
  message. OS-level sandboxing of the build (containerized cargo) is a noted hardening follow-up.
- **Template drift.** Templates rot the instant the reference extensions or the `world` major move. Mitigated
  by rendering *from* the proof-panel/fleet-monitor shape and gating CI on "generate → build → publish green
  for both tiers" — drift breaks the build, loudly. The `world` is pinned in the template to the host major.
- **Toolchain assumptions.** `cargo`, the `wasm32-wasip2` target, `pnpm`, and (per the box's setup) the zigcc
  link wiring must be present. `devkit.inspect` reports toolchain readiness so the wizard pre-flights, and a
  build job fails with a *clear* "missing toolchain" message, not a cryptic cargo error.
- **Signing key custody for the UI path.** Server-side signing means the node holds the publisher seed; that
  is correct for a dev box but must never leak to the page or logs — mirror `lb-secrets` mediation.
- **Long builds + back-pressure on the log stream.** Reuse the jobs + `bus.watch` patterns already shipped;
  don't invent a new streaming path.

## Related

- `extensions-scope.md` — the manifest contract the generator **writes** (id, tier, world, caps, tools).
- `proof-panel-scope.md` / `fleet-monitor` — the shapes the `wasm`/`native` templates are cut from.
- `ui-federation-scope.md` — the federated-page + bridge contract the generated `ui/` page targets.
- `lifecycle-management-scope.md` — takes over after publish (enable/disable/start/stop/delete).
- `reference-extensions-scope.md` — the `kv.*` store + native callback the richer generated extensions use.
- `../jobs/jobs-scope.md` — the durable job the long build runs as (§6.10).
- `../registry/` — the publish/verify/cache/install path the SDK ends in (unchanged).
- `../../public/extensions/dev-flow.md` — the existing build→pack→publish chain this SDK packages.
- README `§6.3` (two tiers), `§6.4` (registry/trust), `§13` (manifest is the contract), `§3` rules 5/7.
