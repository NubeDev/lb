# build/linux/full/

The **full standalone** build type — the `lazybones-shell` ELF with the `full` cargo feature
on. This is the "100% standalone" desktop product: one binary that boots the node, runs the
boot seeders, and **mounts the SSE/HTTP gateway in-process on a loopback port**, so login,
MCP, SSE, the agent catalog, flows, insights — all of it — works with no external node.

`make -C desktop linux-full` builds the binary in the container and copies it here:

```
desktop/build/linux/full/lazybones-shell      ← the ELF (full feature on)
```

Gitignored — regenerated per build, never committed. Only this README is tracked. The thin
shell (Tauri window + 5 IPC commands, no gateway) lands in `../executable/` via
`make linux-executable` — same source, different feature set.

## What's different from `../executable/`

Same binary, one cargo feature: `full = ["desktop", "dep:lb-role-gateway", "dep:lb-authz"]`
(see `ui/src-tauri/Cargo.toml`). At boot (`ui/src-tauri/src/full.rs`), the full mode:

1. Seeds the dev identity (`user:ada` → `workspace-admin` of `acme`, idempotent) so login
   works on a fresh store with zero setup.
2. Runs the catalog seeders (core skills, agent definitions, personas) + the default grants.
3. Spawns the four background reactors (flow / channel-agent / approval / insight-digest).
4. Mounts the gateway on `http://127.0.0.1:8800` (`Gateway::new_live` + `serve_listener`).

The webview talks to that loopback gateway over HTTP — exactly as the browser does against
`make dev`. The UI is built with `VITE_GATEWAY_URL=http://127.0.0.1:8800` baked in (the build
script does this automatically when `full` is on), so `invoke.ts` takes the HTTP path.

## Run it

Same host runtime contract as the thin shell (webkit2gtk-4.1 on the host):

```bash
./desktop/build/linux/full/lazybones-shell
```

On boot, the terminal prints `full: loopback gateway on http://127.0.0.1:8800 (login as user:ada / acme)`.
The window opens against `acme`; the login screen accepts `user:ada` / `acme` (any user the
seed made an admin of the workspace — the dev-login accepts any handle that is a member).

Proof it works (no window needed): boot it under `xvfb-run` and `curl` the loopback gateway —
`make -C desktop smoke-full` does exactly this (login → token → real `POST /mcp/call`).

## Non-goals (recorded, not gaps)

- **Persistent store / signing key.** A fresh in-memory store + ephemeral key per launch
  (like the current `NodeHandle::boot`). State doesn't survive restart; the seeders are
  idempotent so each launch still logs in cleanly. Persistence is a follow-up.
- **Native sidecars** (federation, control-engine). Those need their own binaries + config;
  run `make dev` for them. The core product is fully functional without.
- **Loopback port.** Fixed at `8800` (distinct from dev `8080` so they don't collide).
  Override via `LB_DESKTOP_GATEWAY_ADDR`, but ONLY with a matching UI rebuild (the URL is
  baked at build time).

See [`../../../../docs/scope/desktop/desktop-standalone-backend-scope.md`](../../../../docs/scope/desktop/desktop-standalone-backend-scope.md)
for the full design + decisions.

## Related

- Thin shell (the other build mode): [`../executable/README.md`](README.md).
- Build command + container: [`../../../docker/README.md`](../../docker/README.md).
- Scope: [`../../../../docs/scope/desktop/desktop-standalone-backend-scope.md`](../../../../docs/scope/desktop/desktop-standalone-backend-scope.md).
