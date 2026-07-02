# Operator CLI scope — `lb`, the terminal twin of the shell

Status: **v1 slice shipped** (2026-07-01) — the spine is proven end to end and promoted to
`public/cli/cli.md`; the full typed verb tree + `watch` streams remain follow-up slices (see the v1
slice boundary + the resolved Open questions). Build log: `../../sessions/cli/operator-cli-session.md`;
skill: `../../skills/lb-cli/SKILL.md`.

> Read with: `../auth-caps/api-keys-scope.md` (machine principals — the CLI is its **named first
> consumer**; v1 mints a dev-login token, API keys land as the follow-up), `../auth-caps/auth-caps-scope.md`
> (the token + capability grammar), `../system-map/system-map-scope.md` (the status console the CLI
> surfaces), `../extensions/ext-sdk-scope.md` (the `lb-devkit` library this unifies under `lb devkit`),
> `../../README.md` §6.5 (MCP — the universal contract the CLI speaks), §6.6 (auth/caps), §6.13 (the
> client layer — browser/Tauri/mobile — the CLI joins as a fourth client), §7 (tenancy),
> `../testing/testing-scope.md` §0–§2 (no mocks; mandatory deny + isolation tests).

Today the only ways into a node are the `node` binary (boots + runs a hardcoded demo) and the
browser over the SSE/HTTP gateway. Operators who want to inspect workspace state, drive a workflow,
install an extension, or triage an inbox from a terminal are forced into hand-rolled `curl + jq`
(see `make publish-ext` in the `Makefile`). We want **`lb`** — a first-class operator CLI that is the
*terminal twin* of the React shell: a fourth client of the same gateway surface, holding **no
authority of its own**, denied exactly when the server denies. The whole point: the CLI is **not a
new API, a new permission path, or a second enforcement point** — it is a client of the one
`lb_host::call_tool` chokepoint every other caller already funnels through (browser, extension
pages, the ACP adapter). It adds **zero new MCP verbs, zero new capabilities, zero new tables.**

## v1 slice boundary (what the first ship proves)

The Goals below describe the **full** CLI. v1 is deliberately narrower: it proves the spine end to end
and retires the `curl + jq` flow, and **defers the full typed command tree to follow-up slices**. The
load-bearing v1 surface is:

- `lb login` / `lb whoami` (dev-login token, kept per-workspace) — the front door.
- `lb call <tool> '{json}'` — the universal escape hatch that makes the spine provable in one command
  and reaches *every* verb with zero per-verb wiring.
- **One** typed command (`lb inbox list`) as the proof that typed → `/mcp/call` shaping works.
- `lb ext publish` + `lb devkit sign` — the `make publish-ext` / `lb-pack` retirement.
- `lb local …` — the offline/edge posture.
- The mandatory **capability-deny** + **workspace-isolation** tests (below), across remote **and** local.

Everything else in Goals (the rest of the typed verb set `ws`/`members`/`channels`/`outbox`/`registry`/
`system`/`agent`/`store`/`tags`, plus `watch` SSE streams) grows on the same one client path once the
spine is green — each is a thin addition, not new architecture. Sizing this way keeps v1 shippable and
keeps the "promotes to `public/cli/` once the first slice proves the spine" promise honest.

## Goals

- **One binary, two postures**, mirroring the symmetric-nodes rule (§3.1):
  - *remote* (default) — talks to a running node over the **same** SSE/HTTP gateway the browser uses,
    authenticating with a session token (v1) or an API key (follow-up). Zero new trust surface.
  - *local* (`lb local …`) — embeds `Node::boot()` and calls `call_tool` in-process. This **is** the
    edge/solo posture: the same crates run everywhere, no daemon, fully offline.
- A **universal escape hatch** — `lb call <tool> '{json}'` — that reaches *every* MCP verb through
  `POST /mcp/call`, so the spine is provable in one command and any not-yet-wrapped verb is reachable.
- **Typed commands** over the common operator verbs (`ws`, `members`, `channels`, `inbox`, `outbox`,
  `ext`, `registry`, `system`, `agent`, `store`, `tags`, …) so daily work is ergonomic.
- **Output that makes the wall legible** (PRODUCT.md principle #1): every command prints a
  `ws: …  user: …  role: …` header line so scope is never ambiguous; tables by default, `-o json` for
  scripting; `NO_COLOR` honored; denies rendered honestly (`DENIED  mcp:<tool>:call`), never invented
  as success.
- **Retire the `curl + jq` flow** — `make publish-ext` becomes `lb ext publish`; `lb-pack` folds into
  `lb devkit sign` over the existing `lb-devkit` library (one crypto idiom, unchanged).

## Non-goals

- **No new MCP verbs, capabilities, records, or tables.** The CLI is a pure client; enforcement is the
  server's. (Stated up front because it is the load-bearing simplification — see "How it fits the core".)
- **No new enforcement or trust path.** The CLI never mints authority it wasn't given; in remote mode
  the workspace + caps come from the **token**, in local mode from a **minted principal scoped by `-w`**.
  There is no `if privileged` branch.
- **No new persistence layer.** The CLI stores only its own config (gateway URL, token, default ws) in
  one TOML file. It owns no SurrealDB, no Zenoh. Local mode's store *is* the host's existing store.
- **No MCP wire protocol (JSON-RPC/stdio).** The CLI speaks HTTP to the gateway (the browser path),
  not the ACP stdio protocol — that is `role/acp`'s job, for driving agents from an editor. "MCP" here
  is the **tool namespace** (`inbox.resolve`, `system.overview`), not the wire format.
- **No TUI / full-screen app in v1.** v1 is line-oriented (tables + streams). A `system` status TUI is
  a named follow-up, not the first slice.
- **No API-key issuance from the CLI in v1.** API keys (`api-keys-scope.md`) are themselves still in
  scope; v1 authenticates with the dev-login token. The CLI becomes API keys' first consumer when they
  ship — the credential format `lb_{ws}_{id}_{secret}` is already designed for O(1) ws-scoped lookup.
- **No shell completion / `lb init` scaffolding wizard in v1** (named follow-ups; clap completion is
  cheap to add once the command tree is stable).

## Intent / approach

The CLI is a **client of the existing gateway surface**, structured so the *client library* is testable
in-process against a real gateway and `main.rs` is a thin shell over it (FILE-LAYOUT). The load-bearing
UX — the `ws: … user: … role: …` header that keeps the wall legible — lives in the client library's
`output.rs` (unit-tested), not in `main.rs` formatting. Three decisions, all settled toward reuse:

**1. Remote commands route through the one universal endpoint, `POST /mcp/call`.** The gateway already
exposes it (`role/gateway/src/routes/mcp.rs:42` → `lb_host::call_tool`): `{tool, args}`, authenticates
the session token, re-checks workspace + `mcp:<tool>:call`, returns the tool's JSON. Routing every typed
command through it means **one client code path**, **zero per-verb HTTP wiring**, and **new host verbs
are free** the moment they ship. The typed REST routes (`GET /workspaces`, …) exist and may be used
later for server-shaped output, but v1 does not depend on them.

> *Rejected:* per-verb typed REST routes in v1. They duplicate the verb set on the client, drift from the
> MCP contract, and mean every new host verb needs a matching route + client method before the CLI can
> reach it. `/mcp/call` is the universal contract (rule 7); one path keeps the CLI honest and small.

**2. Two modes share one command tree; the only difference is the transport.** A `Transport` trait with
two impls — `Remote` (a `reqwest` client over the gateway) and `Local` (an in-process `Node` + a minted
`Principal`) — both ending at `call_tool`. Every command is `transport.call(tool, args)`; mode is chosen
by `lb local` (or `--local`), defaulting to remote. This is the symmetric-nodes rule made literal: the
same binary, the same verbs, the same auth path, differing only by where the host runs.

> *Rejected:* two binaries (`lb` remote, `lb-edge` local). It duplicates the command tree and violates
> "one binary, roles are config" (§3.1). A `Transport` seam is the smaller, honest split.

**3. v1 auth is the dev-login token; API keys are the documented upgrade.** `lb login` posts to the
existing `/login` (`role/gateway/src/routes/login.rs`) — the same dev-login the browser uses — and stores
the signed token in the config file, **keyed by the workspace it was minted for**. The token already
carries workspace + caps, verified per request by `session::authenticate`
(`role/gateway/src/session/authenticate.rs:38`), so the wall holds at the front door with **no new auth
code**. Crucially, `POST /mcp/call`'s body is `{tool, args}` only (`role/gateway/src/routes/mcp.rs` —
`McpCall` carries no workspace), so **the server always reads the workspace from the token**. That makes
`-w` a **credential selector, not a workspace override**: `lb -w acme …` loads the `acme` credential; if
none is stored it is a **loud client-side error** ("no session for workspace acme; run `lb login -w
acme`"), never a silent ignore and never a spoofable override. When `api-keys-scope.md` ships, `lb`
becomes its first caller: an `lb_{ws}_{id}_{secret}` credential verifies per request for instant revoke,
and the CLI's per-workspace credential slot becomes a key slot with no command-tree change.

The local mode mints a principal with the **same claim set as the dev-login** (`session::dev_claims`),
scoped by `-w`, so local and remote behave identically — local is not silently more privileged than a
logged-in user (a real parity risk, called out below).

## How it fits the core

- **Tenancy / isolation:** the wall holds **by construction**, not by a check the CLI remembers to make.
  Remote: `POST /mcp/call` carries `{tool, args}` only — **no workspace in the body** (`mcp.rs` `McpCall`) —
  so the server reads the workspace from the verified token (`authenticate.rs`) and a credential for ws A
  **cannot address ws B at all**. `-w` selects which stored credential to use (it cannot override the
  token's workspace); a workspace with no stored credential is a loud client error, not a silent ignore.
  Local: the minted principal's `ws` IS the wall (`call_tool` gate 1). **(Mandatory isolation test: a ws-A
  credential returns only ws-A data even when the operator passes `-w B`; `-w <unstored>` errors loudly;
  local `-w` cannot reach outside the minted principal's workspace.)**
- **Capabilities:** the CLI holds **no authority of its own** — it is exactly as authorized as the token
  it presents. The deny path is the server's opaque `403`; the CLI surfaces it verbatim (`DENIED
  mcp:<tool>:call`), never inventing success. **(Mandatory deny test: an ungranted `lb call` reports the
  server's deny, not a fake ok.)**
- **Placement:** **either** — no `if cloud {…}`. Remote works against any node with the gateway mounted
  (the `cloud`/`workstation` personas); local IS the edge/solo posture (the `appliance`/`workstation`
  personas with no daemon). The difference is config (`--local`), never a code branch.
- **MCP surface (API shape, §6.1):** the CLI **consumes** the existing surface and **exposes nothing
  new** — no new CRUD/get-list/live-feed/batch verbs. Mapping:
  - Each typed command → one existing MCP verb via `POST /mcp/call` (e.g. `lb inbox list` → `inbox.list`,
    `lb system overview` → `system.overview`, `lb ws list` → `workspace.list`).
  - `lb call <tool> <json>` → `POST /mcp/call` directly (the universal escape hatch).
  - **Live feed:** `lb channels watch` / `lb agent watch` consume the **existing** SSE streams
    (`GET /channels/{cid}/stream?token=`, `GET /runs/{job}/stream?token=`) — no new stream, no polling.
  - **Batch / jobs:** N/A for the CLI itself — long batches are already `lb-jobs` on the server; the CLI
    just calls `jobs.*` / `chains.runs.get` and prints status.
- **Data (SurrealDB):** the CLI touches **no platform tables**. Its only state is the config file
  (`gateway_url`, `token`, `default_workspace`). Local mode's store is the host's existing SurrealDB —
  not a second store.
- **Bus (Zenoh):** **N/A** for the CLI itself. It consumes SSE/HTTP (the gateway's translation of motion);
  local mode's bus is the host's.
- **Sync / authority:** the CLI holds **no authority**. Remote = the gateway's authority; local = the
  embedded node's solo authority. Nothing of the CLI's own syncs.
- **Secrets:** the token (v1) / API key (follow-up) in the config file **is** the secret material.
  Written `0600`, **never logged**, never echoed in `--json` output. The `keyring` crate (already a
  workspace dep, key-stack "Secrets") is the documented upgrade path off the plaintext config file —
  same seam `api-keys-scope.md` names for its hash.
- **SDK/WIT impact:** **none** — pure client, no guest ABI, no `Subject` variant, no host change.
- **Skill doc (required):** the CLI **is** an agent-/operator-drivable surface, so the shipping session
  owns `skills/lb-cli/SKILL.md` — a runnable how-to grounded in a live `lb login` → `lb call` →
  `lb ext publish` run (remote **and** `lb local`), covering the `-w` credential-selector semantics and
  the honest-deny output. Named here per SCOPE-WRITTING §6; written on ship, not at scope time. A
  missing/stale skill for this surface is a finding, not a nicety.

## Example flow

1. **Install + login (remote).** `lb login --url http://127.0.0.1:8080 -w acme` posts `/login`
   `{user: dev, workspace: acme}`, stores the token **keyed by workspace** (`acme → <token>`) plus
   `gateway_url` and `default_workspace: acme` in `~/.lazybones/config` (0600). `lb whoami` prints
   `ws: acme  user: dev  role: member  …`.
2. **The spine, one command.** `lb call hello.echo '{"msg":"hi"}'` → `POST /mcp/call` `{tool:
   "hello.echo", args: {…}}` → the server authenticates the token, checks `mcp:hello.echo:call`,
   dispatches, returns the tool's JSON. The CLI prints it (pretty by default, raw with `-o json`).
3. **A typed command.** `lb inbox list` → same `/mcp/call` with `tool: "inbox.list"` → rendered as a
   table with columns the server returned (no invented fields).
4. **Honest deny.** `lb call series.delete '{…}'` with a token lacking `mcp:series.delete:call` → the
   server returns `403`; the CLI prints `DENIED  mcp:series.delete:call` and exits non-zero. It never
   prints a success the server did not grant.
5. **`-w` selects a credential, never overrides the wall.** After `lb login -w acme` and
   `lb login -w beta`, `lb -w beta inbox list` loads the `beta` token and acts there. `lb -w gamma …`
   with no `gamma` credential errors loudly: *"no session for workspace gamma; run `lb login -w
   gamma`."* Passing `-w beta` while the `acme` token is loaded is **not** a cross-workspace hop — the
   `beta` token's own workspace is what reaches the server (there is no ws in the body to override).
6. **Local mode (offline).** `lb local -w acme call hello.echo '{"msg":"hi"}'` embeds `Node::boot()`
   (the `node/src/main.rs:43` pattern), mints a `dev_claims`-shaped principal for `acme`, and calls
   `call_tool` in-process — **no network, no gateway, no daemon**. Identical output to step 2.
7. **Retire curl+jq.** `lb devkit sign hello-v2` (over the `lb-devkit` library `lb-pack` already uses)
   → `lb ext publish` (`POST /extensions`) → `204` ⇒ verified, installed, loaded live. This **is**
   `make publish-ext`, without `jq`.
8. **Stream.** `lb agent watch <job>` opens `GET /runs/{job}/stream?token=…` and prints `RunEvent`s as
   they arrive (pretty by default; NDJSON with `-o json`) — the same stream the UI's `AgentView` renders.

## Testing plan

Real gateway + real `call_tool`, seeded with real records — **no mocks, no `*.fake.ts`**, no second
backend (testing-scope §0). Structure: the **client library** (transport + output shaping + config) is
unit/integration-tested in-process; `main.rs` is a thin shell. Spin up the real gateway the way
`role/gateway/tests/` already does, seed via the real write path, drive the client layer against the
live socket. Mandatory categories from testing-scope §2:

- **Capability-deny (mandatory):** an ungranted `lb call <tool>` reports the server's `403`/deny verbatim
  and exits non-zero — it never fabricates a success. A typed command on a verb the token lacks does the
  same. (The CLI cannot be tricked into *looking* authorized; it only relays the server's decision.)
- **Workspace-isolation (mandatory):** the wall holds by construction — a ws-A credential returns only
  ws-A data even when the operator passes `-w B` (there is no ws in the `/mcp/call` body to honor, so the
  A-token's ws wins; this is **correct**, not a bug — assert it returns A's data, not B's, and not an
  error); `lb -w <unstored>` errors loudly (no silent ignore); local mode's `-w` cannot reach outside the
  minted principal's workspace. Tested across remote + local.
- **Offline/sync:** **local mode runs with no gateway reachable** (the offline posture) and produces
  identical output to remote; remote mode degrades with a clear error when the gateway is unreachable
  (no hang, no fake success).
- **Unit (pure, no IO):** arg parsing (clap); config load/save + `0600` perms; output shaping (table +
  json round-trips a sample tool result); the `lb_{ws}_{id}` token/key prefix parse for the follow-up;
  the workspace/principal header rendering; `NO_COLOR` honoring.
- **Integration (real gateway, real socket):** the spine — `lb login` → `lb call hello.echo` round-trip;
  the deny path; ws-isolation; typed command (`lb inbox list` over a seeded inbox); config persistence
  across invocations; SSE stream consumption (`lb agent watch`/`lb channels watch` over a real stream).
- **`lb devkit sign`:** produces an artifact the registry's `verify_artifact` accepts (round-trip with
  the existing `lb-pack` output as the oracle — same `lb-devkit` library, so byte-identical signing).
- **No hot-reload category** — the CLI is stateless and owns no extension runtime (N/A, stated).

## Risks & hard problems

- **Token/key custody in the config file** — the classic CLI secret risk. A plaintext token at rest is a
  leak vector; one log line echoing it defeats the feature. Discipline: `0600` perms, never-log, never
  echo in output, and the `keyring` upgrade path documented as the resolution. Mirror `api-keys-scope`'s
  secret-discipline review + a test asserting no command emits the token.
- **Invented success / output drift.** The CLI must relay the server's decision faithfully — a command
  that *renders* success while the server denied is worse than no CLI (it lies). Mitigation: surface HTTP
  status + the server's deny string verbatim; shape output defensively over whatever the tool returned;
  never invent fields. The `inbox.list`/`rules.list` envelope bug (STATUS.md debugging note) is exactly
  the class of drift to defend against — shape what the server sends, do not assume a shape.
- **Local-vs-remote parity.** The two modes must feel identical; the trap is local mode minting a principal
  that is silently more privileged than a real login (the demo's broad caps). Mitigation: local mints the
  **same `dev_claims` set** as `/login`, scoped by `-w` — local is not an admin backdoor. Test that local
  denies the same verbs a member token would.
- **Typed-command vs new-verb drift.** If typed commands hardcode result shapes, a host change breaks
  them silently. Mitigation: route through `/mcp/call` and shape defensively; the tool's JSON is the
  contract. Reserve typed REST routes for a later "server-shaped" optimization, not v1.
- **`lb-pack` unification must not break the Makefile.** `make pack` / `trusted-pubkey` call `lb-pack`
  today. Folding signing into `lb devkit sign` must keep the build green: keep `lb-pack` as a thin shim
  over the same `lb-devkit` library in v1, retire it in a later cleanup. (Both already share
  `lb_devkit::sign_artifact` — the shim is trivial.)
- **`-w` as silent override (the load-bearing UX trap).** Because `/mcp/call` carries no workspace in
  the body, a naive `-w B` with an A-token would be **silently ignored** (the A-token's ws wins) — which
  *looks* like it worked on B. That is worse than an error. Resolution (above): `-w` is a **credential
  selector**, and a workspace with no stored credential errors loudly. The isolation test pins this: a
  mismatched `-w` either selects a real B-credential or errors — it never silently acts on the wrong
  workspace.
- **New external deps (clap, tabled).** These are CLI tooling, **not** persistence/auth/bus libraries, so
  they do not violate rules 1/2/9 (one datastore / no sneaking libs / build native on owned seams) — state
  it explicitly like `api-keys-scope` did for its crypto choice. `reqwest` is already a workspace dep
  (registry-host); `clap` (derive) + `tabled` are the only additions.

## Open questions

> **Resolved on the v1 build (2026-07-01).** All of these were settled toward the recommendations
> below; see `../../sessions/cli/operator-cli-session.md` and `../../public/cli/cli.md`. Kept here
> (marked ✅/⏳) so the history of *why* is legible, not deleted.

- ✅ **API keys vs dev-login for v1 auth** — **dev-login token** (reuses `/login`, zero new auth code).
  API keys remain the follow-up (`api-keys-scope.md`); the CLI's per-workspace credential slot becomes
  a key slot with no command-tree change when they ship. **Done.**
- ✅ **`-w` semantics: credential selector, not override.** Implemented exactly as stated — `-w`
  selects the stored credential; an unstored workspace errors loudly (`no session for workspace <ws>;
  run \`lb login -w <ws>\``, exit 4). The isolation tests (remote + local) pin it. **Done.**
- ✅ **Typed REST route vs `/mcp/call`** — v1 routes **every** typed verb through `/mcp/call` (one
  client path, free new verbs); `lb inbox list` shapes defensively over `{items:[…]}` and tells empty
  (`(no rows)`) from an error without a typed envelope. The switch trigger (a validated shape /
  distinct error envelope) is unchanged and per-verb; `/mcp/call` stays the permanent fallback. The
  one exception, as scoped, is `lb ext publish` → `POST /extensions` (its body is a signed `Artifact`,
  not `{tool,args}`). **Done.**
- ✅ **Local-mode default cap set** — **mirrors `session::dev_claims`** (the gateway's own function,
  reused), so local == a real login. The parity deny test confirms local denies the same verbs a
  member token would (e.g. `prefs.set_default`). Revisit if local is used in shared/CI contexts. **Done.**
- ✅ **Config location + env overrides** — `$LB_DIR/config` (TOML), `0600`, with `LB_GATEWAY_URL` /
  `LB_TOKEN` / `LB_WORKSPACE` (+ `LB_DIR`, `LB_DEVKIT_ROOT`) overriding the file. `keyring` documented
  as the phase-2 upgrade. **Done.**
- ⏳ **Streaming output format** — `lb agent watch` / `lb channels watch` (pretty lines / NDJSON with
  `-o json`). **Deferred:** `watch` is a v1 non-goal (SSE streams grow on the same client path in a
  follow-up slice). Recommendation stands; not built this session.
- ✅ **Retire `lb-pack` now or later** — **later.** `lb devkit sign` ships as the unified path (same
  `lb-devkit::sign_artifact`); `lb-pack` is kept as a thin shim, so the Makefile's `pack` /
  `trusted-pubkey` / `publish-ext` stay green. Retire in a later cleanup once docs/Makefile migrate. **Done.**
- ⏳ **README §6.13 note** — the CLI is a fourth client. **Deferred** (additive sentence, must not
  renumber §-references). Flagged as a follow-up; not done this session to avoid touching the
  cross-referenced section numbering.

## Related

- README `§6.5` (MCP — the universal contract the CLI speaks), `§6.6` (auth/caps), `§6.13` (the client
  layer the CLI joins), `§7` (tenancy — the wall the CLI inherits), `§3.1` (symmetric nodes — the
  two-mode shape).
- Sibling scope: `../auth-caps/api-keys-scope.md` (the CLI is its **named first consumer** — v1 uses the
  dev-login token, API keys are the upgrade), `../auth-caps/auth-caps-scope.md` (the token grammar),
  `../system-map/system-map-scope.md` (the status console `lb system` surfaces), `../extensions/ext-sdk-scope.md`
  (the `lb-devkit` library `lb devkit sign` unifies on), `../frontend/admin-console-scope.md` (the UI
  twin of several typed commands).
- Skill: `../../skills/lb-cli/SKILL.md` (written on ship by the implementing session — the runnable
  how-to for driving `lb`, grounded in a live run; see "Skill doc" in "How it fits the core").
- Implementation seams (for the building session): the gateway's universal bridge
  `role/gateway/src/routes/mcp.rs` (`POST /mcp/call` → `lb_host::call_tool`), `role/gateway/src/routes/login.rs`
  (`/login`), `role/gateway/src/session/authenticate.rs` (the verify chokepoint), `lb_host::call_tool`
  (`crates/host/src/tool_call.rs:80`), the local-boot pattern `node/src/main.rs:43`, the `lb-devkit` library
  (`crates/devkit/src/lib.rs`, used by `tools/pack`), and the `Makefile` `publish-ext` flow this retires.
