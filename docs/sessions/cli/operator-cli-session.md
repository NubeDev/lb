# CLI — Operator CLI `lb`, v1 slice (session)

- Date: 2026-07-01
- Scope: ../../scope/cli/operator-cli-scope.md
- Stage: post-S8 (data plane shipped); this is a new client surface on the existing gateway, no stage gate of its own
- Status: done

## Goal

Build the v1 slice of the operator CLI (`lb`) from its scope — the "v1 slice boundary" section, no
more, no less: `lb login`/`whoami`, the universal `lb call`, one typed command (`lb inbox list`),
`lb devkit sign` + `lb ext publish` (the `make publish-ext` retirement), and `lb local …` (offline).
Prove the spine end to end and retire the `curl + jq` flow, with the mandatory capability-deny +
workspace-isolation tests across **both** remote and local, no mocks. The acceptance criterion (from
the scope's "How it fits the core"): the CLI is a **pure client** of the one `lb_host::call_tool`
chokepoint — zero new MCP verbs, capabilities, tables, or enforcement paths — denied exactly when the
server denies.

## What changed

New crate `rust/role/cli/` (`lb-cli` library + `lb` binary), registered in the workspace. The client
library holds all behavior; `main.rs` is a thin shell (parse → run → print → exit code).

- **Transport seam** (`src/transport/`): a `Transport` trait with two impls — `Remote`
  (`remote.rs`, a reqwest client over `POST /mcp/call`) and `Local` (`local.rs`, an in-process
  `Node::boot()` + a minted `Principal`), plus `AnyTransport` (an enum so the dispatcher hands either
  as `impl Transport`). Both end at `lb_host::call_tool`. `publish.rs` is the `ExtPublish` trait
  (publish is a dedicated `POST /extensions` route, not `/mcp/call`).
- **Config** (`src/config.rs`): TOML at `$LB_DIR/config`, per-workspace token map, `0600` on unix,
  env overrides (`LB_GATEWAY_URL`/`LB_TOKEN`/`LB_WORKSPACE`/`LB_DIR`).
- **Header** (`src/header.rs`): the `ws: … user: … role: … mode: …` line, decoded from the token
  (remote) or the minted principal (local). Redacted `Debug` on `Remote` so `{:?}` can never leak the
  token.
- **Output** (`src/output.rs`): table (default) / json, `NO_COLOR`, and **defensive** shaping —
  unwrap the `{items:[…]}` envelope, `(no rows)` for empty, never invent a field.
- **Commands** (`src/commands/`): one file per verb — `login`, `whoami`, `call`, `inbox`, `ext`,
  `devkit`. Each returns a `Printed { header, body }`; `main` owns the one `println!`.
- **Context/dispatch** (`src/context.rs`, `src/dispatch.rs`): the `-w` credential-selector rule
  (loud error on an unstored workspace), remote-vs-local resolution (a flag choice, never `if cloud`),
  and the `lb local <cmd>` argv-prefix sugar (rewritten to `--local` before clap parses — a recursive
  `Local` subcommand overflowed clap's help generation).
- **Sign** (`src/sign.rs`): folds over `lb_devkit::sign_artifact` — the same library `lb-pack` uses,
  so `lb devkit sign` is byte-identical and verifies by construction.
- **Error/exit codes** (`src/error.rs`): `Denied` renders `DENIED  mcp:<tool>:call` (exit 3);
  `NoCredential` (4), `Transport` (5), `BadInput` (2), `Other` (1).
- **One host-crate touch**: added `lb_auth::claims_unverified(token) -> Option<Claims>`
  (`rust/crates/auth/src/token.rs`) — a client-side JWT introspection read for the header. NOT an
  authorization path (the server re-verifies every request); it only labels the caller's own terminal.
- New workspace deps: `clap` (derive), `tabled`, `dirs`, `tempfile` (dev). These are CLI ergonomics,
  not persistence/auth/bus libraries (scope Risk "New external deps").

## Decisions & alternatives

- **`AnyTransport` enum over `Box<dyn Transport>`.** The trait's `async fn call` is not cleanly
  object-safe-with-`Send`; a two-variant enum is smaller, faster, and avoids the boxing dance. (First
  attempt used a `TransportDyn` boxed trait — it hit `Send` + method-ambiguity errors; the enum is the
  honest split.)
- **`lb local <cmd>` via argv rewrite, not a recursive clap subcommand.** A `Local { command:
  Box<Command> }` variant made clap's derive help-generation recurse infinitely (stack overflow on
  `--help`). Rewriting a leading `local` arg to the global `--local` flag in `Cli::parse_argv` is one
  place, no recursion.
- **`Principal::routed` for the local principal.** Local mode is in-process co-trust — `routed` is the
  honest constructor for that (the node runs the principal in-process; the ws-scoped wall still
  holds). It carries the full `dev_claims` caps (parity with a real login), so local denies the same
  verbs a member token would — not an admin backdoor.
- **`lb ext publish` uses `POST /extensions`, not `/mcp/call`.** Its body is a signed `Artifact`, not
  a `{tool, args}` call — the scope maps it there explicitly. Every OTHER verb routes through
  `/mcp/call` (one client path, free new verbs).
- **Header on stderr, body on stdout.** So `-o json | jq` gets a clean stream and the context line is
  still visible. Rejected: mixing both on stdout (breaks piping).
- **Added `claims_unverified` to `lb-auth`** rather than reimplementing base64url decode in the CLI —
  keeps the JWT idiom in one crate. It grants nothing (the server verifies every request), so it is
  not a new trust surface.

## Tests

`rust/role/cli/tests/` — the client library driven **in-process against a REAL gateway on a real
socket** (spawned the way `role/gateway/tests/` does), seeded via the real write path
(`lb_host::record_inbox`). No mocks, no `*.fake` (CLAUDE §9, testing §0). Mandatory categories, both
remote AND local:

- **Capability-deny** — remote: an ungranted `call` relays the server's `403` as `DENIED
  mcp:inbox.list:call`, exit non-zero (`remote_test.rs`). Local: parity — `prefs.set_default`
  (admin-only, ungranted to the dev member) is denied (`local_test.rs`). Publish without the cap →
  `403` (`ext_publish_test.rs`).
- **Workspace-isolation** — remote: seed the same channel+id in ws A and ws B on one node; an A-token
  returns A's data, never B's, even when it is *also* the selected credential (the A-token's ws wins by
  construction). Local: an `acme` principal and a `beta` principal over one shared store each see only
  their own workspace. `-w <unstored>` errors loudly (`config_persistence_test.rs`, `context.rs` unit).
- **Offline** — local runs with no gateway reachable and matches remote shape; remote fails with a
  clear `Transport` error (not a hang, not a fake success) when the gateway is down.
- **Unit** — config load/save + `0600`; output shaping (table + json round-trip, envelope unwrap,
  empty → `(no rows)`); `NO_COLOR`; header rendering + "the header never contains the token"; format
  parse.
- **`devkit sign` round-trip** — the signed artifact passes `lb_registry::verify_artifact` (the
  `lb-pack` oracle, same library); a tampered artifact fails (`sign_test.rs`).
- **Token custody** — "no command output ever emits the stored token" after a real login
  (`config_persistence_test.rs`).

### Green output

`cargo test -p lb-cli` — 38 tests, all passing:

```
running 21 tests   (lib unit: config, header, output, context)
test result: ok. 21 passed; 0 failed; 0 ignored

running 3 tests   (config_persistence_test)
test selecting_an_unstored_workspace_errors_loudly ... ok
test login_persists_a_credential_a_later_command_loads_and_uses ... ok
test no_command_output_ever_emits_the_stored_token ... ok
test result: ok. 3 passed; 0 failed

running 2 tests   (ext_publish_test)
test ext_publish_signs_and_installs_over_the_real_gateway ... ok
test ext_publish_without_the_cap_is_denied_server_side ... ok
test result: ok. 2 passed; 0 failed

running 4 tests   (local_test)
test local_denies_a_verb_outside_the_dev_claims_set ... ok
test local_mode_runs_with_no_gateway_reachable ... ok
test local_output_matches_remote_shape ... ok
test local_dash_w_cannot_reach_outside_the_minted_principals_workspace ... ok
test result: ok. 4 passed; 0 failed

running 6 tests   (remote_test)
test remote_mode_fails_clearly_when_the_gateway_is_down ... ok
test an_ungranted_call_relays_the_servers_deny_and_never_fakes_success ... ok
test a_ws_a_token_returns_only_ws_a_data_even_when_targeting_b ... ok
test login_then_call_round_trips_over_the_real_gateway ... ok
test the_header_reflects_the_tokens_workspace_and_never_leaks_the_token ... ok
test typed_inbox_list_shapes_a_seeded_inbox ... ok
test result: ok. 6 passed; 0 failed

running 2 tests   (sign_test)
test devkit_sign_produces_an_artifact_verify_artifact_accepts ... ok
test a_tampered_artifact_fails_verification ... ok
test result: ok. 2 passed; 0 failed
```

Full workspace (the gate): `cargo fmt --check` clean; `cargo build --workspace` exit 0;
`cargo test --workspace` — **1200 passed, 0 failed** across every crate (the CLI's 38 among them; the
`lb-auth` suite green with the added `claims_unverified`). `lb-pack` still builds + runs (the shim is
intact, Makefile green).

### Live end-to-end run (grounds the skill)

Against a **real** node with the gateway mounted (`LB_GATEWAY_ADDR=127.0.0.1:18080`,
`LB_TRUSTED_PUBKEYS=<dev key>`):

```
$ lb login --url http://127.0.0.1:18080 -w acme
ws: acme  user: user:ada  role: member  mode: remote
logged in to http://127.0.0.1:18080 as user:ada (workspace acme); credential stored in "…/.lazybones/config"

$ lb call inbox.list '{"channel":"ops"}' -o json
{ "items": [] }

$ lb devkit sign hello-v2 --out /tmp/hello.artifact.json
signed hello v0.2.0 → /tmp/hello.artifact.json  (publisher: dev-publisher)

$ lb ext publish /tmp/hello.artifact.json
published hello v0.2.0 — verified, installed, loaded live

$ lb call prefs.set_default '{}'; echo $?
DENIED  mcp:prefs.set_default:call
3

$ lb -w beta inbox list ops; echo $?
no session for workspace beta; run `lb login -w beta`
4

# offline, no gateway:
$ lb local -w acme whoami
ws: acme  user: user:ada  role: member  mode: local

$ lb local -w acme call inbox.list '{"channel":"general"}'
(no rows)

$ lb local -w acme call prefs.set_default '{}'; echo $?
DENIED  mcp:prefs.set_default:call
3
```

## Debugging

None — nothing broke that warranted a `debugging/` entry. Two build-time missteps fixed in-session
(not runtime bugs): (1) a `Box<dyn Transport>` design that failed the `Send`/object-safety check →
replaced with the `AnyTransport` enum; (2) a recursive `Local { Box<Command> }` clap subcommand that
overflowed help generation → replaced with the argv-prefix rewrite. Both are recorded under
"Decisions" above.

## Public / scope updates

- Promoted the shipped truth to `docs/public/cli/cli.md` (filled the TODO stub) and added the row to
  `docs/public/SCOPE.md`.
- Wrote `docs/skills/lb-cli/SKILL.md` — the runnable how-to, grounded in the live run above.
- Resolved the scope's open questions (all settled toward the decisions the build made — see the
  scope's updated "Open questions" section): v1 auth = dev-login token; `-w` = credential selector;
  `/mcp/call` for all typed verbs; local caps = `dev_claims`; config at `$LB_DIR/config` + env
  overrides; `lb-pack` kept as a shim. The README §6.13 note is deferred (additive, not renumbering) —
  left as a flagged follow-up in the scope.

## Skill docs

`docs/skills/lb-cli/SKILL.md` — written this session, grounded in the live `lb login` → `lb call` →
`lb devkit sign` → `lb ext publish` run (remote) and the `lb local …` run (offline), covering the
`-w` credential-selector semantics and the honest-deny output. Required by the scope's "Skill doc"
clause.

## Dead ends / surprises

- `POST /extensions` accepts **two** body shapes (a full `Artifact` OR a `{path}` devkit shortcut).
  The CLI sends the `Artifact` (the `lb devkit sign` output), so it works against any node trusting
  the publisher key — not just one with the local devkit root.
- The built-binary filename convention is `{extension-id}_ext.wasm` (from the manifest `id`), not the
  crate/dir name — the sign path and the gateway's devkit shortcut both derive it from `id`. The tests
  stage the real wasm under that name.
- `hello.echo` is NOT in `dev_claims` (it's a demo tool cap), so `lb call hello.echo` after publish is
  an honest `DENIED` for the dev member — correct behavior, but a reminder that a working extension is
  still gated by the caller's caps.

## Follow-ups

- The full typed verb set (`ws`/`members`/`channels`/`outbox`/`registry`/`system`/`agent`/`store`/
  `tags`) + `watch` SSE streams — thin additions on the same client path (scope non-goals of v1).
- API-key auth (`lb_{ws}_{id}_{secret}`) when `api-keys-scope` ships — the CLI is its first consumer.
- `keyring` for the token at rest (phase-2 upgrade off the plaintext config).
- Shell completion (clap makes it cheap once the tree is stable) and a `system` status TUI.
- The README §6.13 "fourth client" sentence (additive; must not renumber) — flagged in the scope.
- STATUS.md updated: new "Operator CLI (`lb`) v1" slice marked shipped.
```
