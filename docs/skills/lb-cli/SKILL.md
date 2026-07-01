---
name: lb-cli
description: >-
  Drive a Lazybones node from the terminal with `lb`, the operator CLI — the terminal twin of the
  React shell. Use when a task says "log in / call an MCP tool / list an inbox / publish an extension
  from the command line", "run lb …", "operate a node without curl+jq", "script against the gateway",
  or "work offline / on the edge with the embedded node". Covers `lb login` / `lb whoami`, the
  universal `lb call <tool> '{json}'` escape hatch, the typed `lb inbox list`, `lb devkit sign` +
  `lb ext publish` (the `make publish-ext` retirement), and `lb local …` (offline, in-process). It is
  a PURE client of the one `POST /mcp/call` chokepoint — it holds no authority, and is denied exactly
  when the server denies (`DENIED  mcp:<tool>:call`, non-zero exit).
---

# Operating a node from the terminal (`lb`)

`lb` is the **operator CLI** — a fourth client (besides browser / Tauri / mobile) of the same node
gateway, holding **no authority of its own**. Everything it does funnels through the one
`lb_host::call_tool` chokepoint every other caller uses, so a command is denied exactly when the
server denies. It adds **zero new MCP verbs, capabilities, or tables** — it is a client, the
enforcement is the server's.

Two postures, one command tree (the symmetric-nodes rule made literal — only the transport differs):

- **remote** (default) — talks to a running node over its HTTP gateway with a session token.
- **local** (`lb local …` / `--local`) — embeds `Node::boot()` and calls `call_tool` in-process. No
  daemon, no network, fully **offline**. This is the edge/solo posture.

Build it: `cd rust && cargo build -p lb-cli --bin lb` → `rust/target/debug/lb`.

## Every command prints a legibility header

Before any result, `lb` prints `ws: … user: … role: … mode: …` to **stderr** — so the workspace
(the hard wall) is never ambiguous, and remote-vs-local is visible. The result body goes to
**stdout**, so `-o json` is a clean, header-free stream you can pipe. Denials print
`DENIED  mcp:<tool>:call` to stderr and exit **non-zero** — the CLI never fabricates a success the
server did not grant.

```
$ lb whoami
ws: acme  user: user:ada  role: member  mode: remote      # ← stderr (context)
{ "user": "user:ada", "workspace": "acme", "role": "member", "caps": [ … ] }   # ← stdout (data)
```

Global flags (all commands): `-w/--workspace` (credential selector), `-o/--output table|json`,
`--url <gateway>`, `--local`. `NO_COLOR` is honored.

## 1. `lb login` — the front door

```bash
lb login --url http://127.0.0.1:8080 -w acme          # dev-login user defaults to user:ada
# ws: acme  user: user:ada  role: member  mode: remote
# logged in to http://127.0.0.1:8080 as user:ada (workspace acme); credential stored in "…/.lazybones/config"
```

`login` POSTs the dev-login `{user, workspace}` to the gateway's `/login` (the same the browser
uses), and stores the signed token **keyed by the workspace it was minted for**, in
`$LB_DIR/config` (TOML, `0600`). The token carries the workspace + caps and is verified per request
server-side — the wall holds at the front door with no new auth code. Override the user with
`--user user:bo`.

**The token is secret material**: written `0600`, never logged, never echoed in any command's
output (including `-o json`). Don't paste it into shell history — let `login` store it.

## 2. `lb call <tool> '{json}'` — the universal escape hatch

Reaches **every** MCP verb through `POST /mcp/call` with zero per-verb wiring — the spine, provable
in one command:

```bash
lb call inbox.list '{"channel":"ops"}' -o json
# ws: acme  user: user:ada  role: member  mode: remote
# { "items": [] }

lb call series.list '{}' -o json
# { "series": [] }
```

Args default to `{}` if omitted. A malformed JSON args string is a clean bad-input error (exit 2),
never a call with garbage. The workspace is NEVER in the body — the server reads it from your token.

### Honest deny (the load-bearing behavior)

A verb your token lacks is refused by the server (`403`); `lb` relays that verbatim and exits
non-zero — it does **not** invent a success:

```bash
lb call prefs.set_default '{}'          # admin-only; a dev member lacks it
# DENIED  mcp:prefs.set_default:call
echo $?                                 # 3
```

Exit codes a script can branch on: `0` ok · `2` bad input · `3` **DENIED** · `4` no stored
credential for `-w` · `5` gateway unreachable.

## 3. `lb inbox list <channel>` — the one typed command

Typed sugar over `inbox.list` (proof that typed → `/mcp/call` shaping works). It shapes whatever the
server returns **defensively** — an empty inbox reads as `(no rows)`, never an error, never an
invented shape:

```bash
lb inbox list ops
# ws: acme  user: user:ada  role: member  mode: remote
# (no rows)
```

Table by default (columns come from the items the server returns); `-o json` for the raw envelope.

## 4. `lb devkit sign` + `lb ext publish` — retire `make publish-ext`

`devkit sign` folds `lb-pack` into the CLI (the SAME `lb-devkit` signing library — byte-identical
artifacts the registry verifies). `ext publish` uploads a signed `Artifact` to `POST /extensions`,
which verifies-before-stores and installs + loads it live.

```bash
# Sign a BUILT extension (under $LB_DEVKIT_ROOT, default rust/extensions) into an artifact:
lb devkit sign hello-v2 --out /tmp/hello.artifact.json
# ws: acme  user: user:ada  role: member  mode: remote
# signed hello v0.2.0 → /tmp/hello.artifact.json  (publisher: dev-publisher)

# Publish it (needs mcp:ext.publish:call + the gateway trusting the publisher key):
lb ext publish /tmp/hello.artifact.json
# published hello v0.2.0 — verified, installed, loaded live
```

Or in one step — `lb ext publish` accepts an extension **name/dir** too, signing on the fly:

```bash
lb ext publish hello-v2      # sign hello-v2 then publish it
```

The gateway must **trust the publisher key** (as `LB_TRUSTED_PUBKEYS` does in dev). Get the trust
line with the shim: `lb-pack pubkey $LB_DEVKIT_ROOT/keys/dev-publisher.key --key-id dev-publisher`.
A publish without `mcp:ext.publish:call` is `DENIED  mcp:ext.publish:call` (exit 3); a
verification failure (tampered / foreign key) is a bad-input error (exit 2) — nothing stored.

> `lb-pack` still exists as a thin shim over the same `lb-devkit` library, so the Makefile's `pack`
> / `trusted-pubkey` / `publish-ext` targets stay green. `lb devkit sign` + `lb ext publish` are the
> unified path that retires the `curl + jq` flow; `lb-pack` is retired in a later cleanup.

## 5. `lb local …` — offline, in-process (the edge posture)

Prefix any command with `lb local` (or pass `--local`) to run it against an **embedded** node — no
gateway, no network, no login. It mints a principal with the **same `dev_claims` claim set** a real
login issues, scoped by `-w`, so local is **not** an admin backdoor — it denies the same verbs a
member token would:

```bash
lb local -w acme whoami
# ws: acme  user: user:ada  role: member  mode: local

lb local -w acme call inbox.list '{"channel":"general"}'
# ws: acme  user: user:ada  role: member  mode: local
# (no rows)

lb local -w acme call prefs.set_default '{}'      # parity deny — same as a member token
# DENIED  mcp:prefs.set_default:call    (exit 3)
```

Local output is identical to remote (same claim set, same dispatch) — only the header's `mode:`
differs. The `-w` scopes the minted principal's workspace, which **is** the wall; a local `-w acme`
principal cannot reach ws `beta`'s data.

## `-w` is a credential SELECTOR, never a workspace override

Because `POST /mcp/call` carries no workspace in the body, `-w` cannot override the token's
workspace — the token's own workspace always reaches the server. `-w` picks **which stored
credential** to present. A workspace with no stored credential is a **loud** error, never a silent
hop to the wrong one:

```bash
lb -w beta inbox list ops
# no session for workspace beta; run `lb login -w beta`
echo $?    # 4
```

So after `lb login -w acme` and `lb login -w beta`, `lb -w beta …` acts in beta and `lb -w acme …`
in acme — each with its own token. Passing `-w beta` while an acme token is loaded is not a
cross-workspace hop: the beta token's own workspace is what reaches the server.

## Config & env

Config lives at `$LB_DIR/config` (TOML; `LB_DIR` defaults to `~/.lazybones` — the same root the
Makefile uses). It holds the gateway URL, the default workspace, and the per-workspace token map.
Env overrides (mirroring the node's env-config style) win over the file:

| Env | Overrides |
|---|---|
| `LB_GATEWAY_URL` | the gateway base URL |
| `LB_TOKEN` | a raw session token (CI injects here, out of band) |
| `LB_WORKSPACE` | the selected/default workspace |
| `LB_DIR` | the `.lazybones` config root |
| `LB_DEVKIT_ROOT` | where `devkit sign` / `ext publish` resolve an extension name |

`keyring` (OS keychain) for the token is a documented phase-2 upgrade off the plaintext file; API
keys (`lb_{ws}_{id}_{secret}`) become the credential when they ship — the CLI is their first
consumer, with no command-tree change.

## End-to-end (a real session)

```bash
LB=rust/target/debug/lb
URL=http://127.0.0.1:8080

# remote: log in, prove the spine, publish an extension
$LB login --url $URL -w acme
$LB call inbox.list '{"channel":"ops"}' -o json          # { "items": [] }
$LB devkit sign hello-v2 --out /tmp/a.json               # signed hello v0.2.0 → …
$LB ext publish /tmp/a.json                              # published hello v0.2.0 — installed, live
$LB call prefs.set_default '{}'; echo $?                 # DENIED … ; 3  (honest deny)

# offline: the same commands, no gateway
$LB local -w acme whoami
$LB local -w acme call inbox.list '{"channel":"general"}'   # (no rows)
```

## Non-negotiable rules this encodes

- **Pure client, no new trust path** (rules 5/7). `lb` mints no authority — remote's caps come from
  the **token**, local's from a `dev_claims`-shaped **minted principal**. There is no `if privileged`
  branch; the deny path is the server's opaque `403`, surfaced verbatim.
- **Workspace is the hard wall** (rule 6). Remote: the ws comes from the token, not the `/mcp/call`
  body — a ws-A credential cannot address ws-B. Local: the minted principal's ws IS the wall. `-w`
  selects a credential; it never overrides.
- **Symmetric nodes** (rule 1). One binary, one command tree; remote vs local is config (`--local`),
  never a code branch.
- **Honest output** (PRODUCT.md #1). Every command states the wall (`ws/user/role`); a deny is
  rendered `DENIED  mcp:<tool>:call` and exits non-zero — never a fabricated success. Output is
  shaped defensively over what the tool returned, never an invented field.
- **One signing idiom** (S7). `devkit sign` uses the same `lb-devkit` library the registry verifies
  with — a signed artifact verifies by construction; `lb-pack` stays a shim over it.

## Test it (no mocks — the real chain)

Rule 9: nothing that can run in-process is mocked. The CLI's client library is tested **in-process
against a REAL gateway on a real socket**, seeded via the real write path — `rust/role/cli/tests/`:

- `remote_test.rs` — login → call round-trip, capability-deny (server `403` → `DENIED`, exit
  non-zero), workspace-isolation (a ws-A token returns only ws-A data even when passed `-w B`), the
  typed `inbox list` over a seeded inbox, and offline (a down gateway → clear `Transport` error).
- `local_test.rs` — offline operation, deny parity (local denies the same verbs a member token
  would), and local `-w` workspace-isolation on one shared store.
- `ext_publish_test.rs` — sign → publish over the real gateway (the tool becomes callable), and the
  publish-without-cap deny.
- `sign_test.rs` — `devkit sign` produces an artifact `lb_registry::verify_artifact` accepts (the
  `lb-pack` oracle); a tampered artifact fails.
- `config_persistence_test.rs` — a login-stored credential a later command loads + uses; the
  `-w <unstored>` loud error; and the "no command output ever emits the token" regression.

Run them: `cd rust && cargo test -p lb-cli`.

## Gotchas

- **Header on stderr, body on stdout.** `-o json | jq` works because the `ws:…` header is on stderr.
  Redirect `2>/dev/null` if you want only the body.
- **A DENY is not a bug.** `DENIED  mcp:<tool>:call` (exit 3) means the server refused — check your
  token's caps and workspace. It is indistinguishable from "no such tool" (opaque, by design).
- **`-w <ws>` needs a stored credential** (or `LB_TOKEN`) for that workspace, else exit 4. Log in
  first: `lb login -w <ws>`.
- **`lb login` is remote-only.** Local mode mints in-process; `lb local login` errors loudly.
- **`ext publish` trust is the gateway's.** A CLI-signed artifact is verified against the *gateway's*
  `LB_TRUSTED_PUBKEYS` — you cannot self-trust onto someone else's node. In `lb local` mode you sign
  and publish on your own node, which trusts your own dev key.
- **`devkit sign` needs a BUILT extension.** It reads the built binary
  (`target/wasm32-wasip2/release/<id>_ext.wasm` for wasm) under `$LB_DEVKIT_ROOT`; run the ext's
  `build.sh` first.

## Related

- Scope: `docs/scope/cli/operator-cli-scope.md` (the ask + the `-w` credential-selector resolution).
- Public: `docs/public/cli/cli.md` (the shipped truth).
- Session: `docs/sessions/cli/operator-cli-session.md` (the build log + green tests).
- Implementation: `rust/role/cli/` (client lib + `lb` binary); the seams it uses —
  `rust/role/gateway/src/routes/mcp.rs` (`/mcp/call`), `…/routes/login.rs`, `…/routes/ext.rs`
  (`/extensions`), `rust/crates/host/src/tool_call.rs` (`call_tool`), `rust/crates/devkit/`
  (signing). Sibling surface: `docs/skills/extensions/SKILL.md` (the browser/API extension flow this
  CLI mirrors).
```
