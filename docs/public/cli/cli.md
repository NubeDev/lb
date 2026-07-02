# Operator CLI (`lb`)

Status: **shipped (v1 slice)** — the spine end to end; retires the `curl + jq` extension flow. See
`../../scope/cli/operator-cli-scope.md` (the ask), `../../sessions/cli/operator-cli-session.md` (the
build log), and `../../skills/lb-cli/SKILL.md` (the runnable how-to).

`lb` is the **terminal twin of the React shell**: a fourth client (besides browser / Tauri / mobile)
of the same node gateway, holding **no authority of its own**. Every command funnels through the one
`lb_host::call_tool` chokepoint every other caller uses, so `lb` is denied exactly when the server
denies. It adds **zero new MCP verbs, capabilities, tables, or enforcement paths** — a pure client.

## Two postures, one command tree

The symmetric-nodes rule (§3.1) made literal: one binary, one verb set, differing only by transport.

- **remote** (default) — talks to a running node over its HTTP gateway with a session token, over
  the **same** `POST /mcp/call` bridge the browser uses.
- **local** (`lb local …` / `--local`) — embeds `Node::boot()` and calls `call_tool` in-process. No
  daemon, no network, fully **offline**. This is the edge/solo posture; local mints a principal with
  the **same `dev_claims` claim set** a real login issues (parity — not an admin backdoor).

Build: `cd rust && cargo build -p lb-cli --bin lb` → `rust/target/debug/lb`.

## The v1 command set

| Command | What | Path |
|---|---|---|
| `lb login [--url U] -w WS [--user U]` | Dev-login; store the token per-workspace (`0600`, never logged). | `POST /login` |
| `lb whoami` | Who / which ws / role / caps — from the token (remote) or minted principal (local). Never the token. | offline read |
| `lb call <tool> '{json}'` | The universal escape hatch — reaches every MCP verb. | `POST /mcp/call` |
| `lb inbox list <channel>` | The one typed command (proof of typed → `/mcp/call` shaping). | `inbox.list` via `/mcp/call` |
| `lb devkit sign <name\|dir> [--out F]` | Sign a built extension into the `Artifact` JSON the registry verifies (the `lb-pack` fold). | `lb-devkit::sign_artifact` |
| `lb ext publish <artifact.json \| name\|dir>` | Publish a signed artifact (sign on the fly if given a dir) — the `make publish-ext` retirement. | `POST /extensions` |
| `lb local <cmd>` | Run any command against the embedded in-process node (offline). | `call_tool` in-process |

Global flags (all commands): `-w/--workspace` (credential selector), `-o/--output table|json`,
`--url <gateway>`, `--local`. `NO_COLOR` honored.

## Output that makes the wall legible

Every command prints a `ws: … user: … role: … mode: …` header to **stderr** (context), and the
result body to **stdout** (data) — so `-o json` is a clean, header-free stream to pipe. A capability
denial is rendered `DENIED  mcp:<tool>:call` to stderr and **exits non-zero** — the CLI relays the
server's decision, it never fabricates a success. Output is shaped **defensively** over whatever the
tool returned (an empty list is `(no rows)`, not an error; the `{items:[…]}` envelope is unwrapped;
no field is invented).

Exit codes (scriptable): `0` ok · `2` bad input · `3` **DENIED** · `4` no stored credential for `-w`
· `5` gateway unreachable.

## `-w` is a credential selector, never a workspace override

Because `POST /mcp/call` carries no workspace in its body, the token's own workspace always reaches
the server. `-w` selects **which stored credential** to present; it cannot override the wall. A
workspace with no stored credential is a **loud** client-side error
(`no session for workspace <ws>; run \`lb login -w <ws>\``, exit 4), never a silent hop. In local
mode, `-w` scopes the minted principal's workspace, which **is** the wall.

## Config & secrets

The CLI's only persistence is its own config at `$LB_DIR/config` (TOML; `LB_DIR` defaults to
`~/.lazybones`). It holds the gateway URL, the default workspace, and the **per-workspace token
map**. The token is secret material: written `0600`, never logged, never echoed in output (a
regression test asserts no command emits it). Env overrides win over the file: `LB_GATEWAY_URL`,
`LB_TOKEN`, `LB_WORKSPACE`, `LB_DIR`, `LB_DEVKIT_ROOT`. `keyring` is a documented phase-2 upgrade;
API keys (`lb_{ws}_{id}_{secret}`) become the credential when they ship, with no command-tree change.

## Architecture

A `Transport` trait with two impls — `Remote` (a `reqwest` client over the gateway) and `Local` (an
in-process `Node` + a minted `Principal`) — both ending at `call_tool`. Every command is
`transport.call(tool, args)`; the mode is a config/flag choice (`AnyTransport`), never an `if cloud`
branch. The client **library** (transport + config + output shaping + the `ws/user/role` header)
lives in `lb_cli` and is unit-/integration-tested in-process against a real gateway; `main.rs` is a
thin shell. One responsibility per file (FILE-LAYOUT).

`lb ext publish` is the one command that uses a dedicated route (`POST /extensions`) rather than
`/mcp/call` — because its body is a signed `Artifact`, not a `{tool, args}` call. `lb devkit sign`
folds over the existing `lb-devkit` library (byte-identical to `lb-pack`); `lb-pack` stays as a thin
shim so the Makefile's `pack` / `trusted-pubkey` / `publish-ext` targets stay green.

## Tested (no mocks — the real chain)

The client library is driven **in-process against a real gateway on a real socket**, seeded via the
real write path (`rust/role/cli/tests/`): the login→call spine, capability-deny (server `403` →
`DENIED`, non-zero exit), workspace-isolation (a ws-A token returns only ws-A data even when passed
`-w B`; local `-w` cannot cross), offline (local runs with no gateway; remote fails clearly when the
gateway is down), config persistence across invocations, the `devkit sign` → `verify_artifact`
round-trip (the `lb-pack` oracle), and a "no command emits the token" regression. Run:
`cd rust && cargo test -p lb-cli`.

## Deferred to follow-up slices (explicit non-goals of v1)

The full typed verb set (`ws` / `members` / `channels` / `outbox` / `registry` / `system` / `agent`
/ `store` / `tags`) and the `watch` SSE streams grow on the same one client path — each a thin
addition, not new architecture. API-key issuance, shell completion, and a `system` status TUI are
named follow-ups. See the scope's Goals / Non-goals.

## Related

- Scope: `../../scope/cli/operator-cli-scope.md`. Skill: `../../skills/lb-cli/SKILL.md`. Session:
  `../../sessions/cli/operator-cli-session.md`.
- README §6.5 (MCP — the contract the CLI speaks), §6.6 (auth/caps), §6.13 (the client layer the CLI
  joins as a fourth client), §7 (tenancy — the wall it inherits), §3.1 (symmetric nodes).
- Seams: `rust/role/gateway/src/routes/mcp.rs` (`/mcp/call`), `…/routes/login.rs`, `…/routes/ext.rs`
  (`/extensions`), `rust/crates/host/src/tool_call.rs` (`call_tool`), `rust/crates/devkit/`.
