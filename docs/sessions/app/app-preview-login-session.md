# app — preview login flow: fresh-store + prefill + singleton regression (session)

- Date: 2026-07-04
- Scope: follow-on to [app-shell-session.md](app-shell-session.md) (the browser preview + the
  `client.ts` singleton fix carried as a deferred regression there)
- Stage: post-S10 app slice 1 of 3 (shell), preview hardening
- Status: **done** — the prefilled ada/acme login flows straight through to Channels in the
  browser preview; the deferred singleton regression test is in and green (13/13).

## What this session set out to do

The prior session left the browser preview (`app/shell/web/` + `vite.config.web.mts`, run via
`make -C app dev`) working but with two open threads: (1) a prefilled `ada/acme` login was 403ing
`"not a member of any workspace"` intermittently, and the `rm -rf` store-reset in `app/Makefile`
didn't reliably fix it; (2) the `client.ts` "stuck on login screen" singleton fix had no
regression test. Also decide keep-vs-park the preview. **Decision: keep it** — it works and is the
fast UI loop; the fix + test are in the repo.

## Root causes (three, all found and fixed)

### 1. The 403 was a PORT collision, not a store-persistence bug

The suspicion was that `test_gateway` persisted `acme` across `rm -rf`. It does **not**.
`test_gateway` boots `lb_host::Node` with **no `LB_STORE_PATH`**, so `host::boot::open_store`
takes the `Store::memory()` branch ([boot.rs:222](../../../rust/crates/host/src/boot.rs)) — an
**in-memory** store. Every boot is already pristine; `rm -rf` of `LB_DIR` was irrelevant (that dir
only holds node keys/config, never the store).

The real cause: the app Makefile's `GW_PORT` was **8080**, the SAME port the root `make dev` node
uses. That node is a **persistent** on-disk node (`LB_STORE_PATH` set) whose store already had
`acme` claimed by a different user. When both were up, the app's `test_gateway` failed to bind 8080
(silently — `axum::serve` errored and the process exited), so the preview kept talking to the
**root node's** memory → `ada` isn't a member of its `acme` → 403. A never-used port always gave
200 because nothing else held it. Confirmed by `readlink /proc/<pid>/exe` on the 8080 holder → it
was `target/debug/node`, not `test_gateway`.

**Fix** ([app/Makefile](../../../app/Makefile)):
- `GW_PORT` moved to **8087** (the preview gets its OWN throwaway in-memory node, off the root
  node's 8080).
- `make gateway` now frees its port **only if a `test_gateway` holds it** — it inspects
  `/proc/<pid>/exe` and **refuses to kill** a non-`test_gateway` (e.g. the root `make dev` node),
  erroring with a `GW_PORT=<free>` hint instead of clobbering another terminal's node.
- Dropped the misleading `rm -rf $(LB_DIR)` (the store was never there) and the `KEEP_STORE` flag
  (in-memory is fresh-by-construction; there is nothing to keep).
- `make kill` now targets `[v]ite.*vite.config.web.mts`, not a bare `[v]ite`, so it no longer kills
  the web-shell's `ui/` vite running in another terminal.
- `dev-defaults.ts` + `web/index.web.tsx` default node URL updated 8080 → **8087** to match.

### 2. The prefill never populated in the preview (`__DEV__` hardcoded false)

Even on a clean node the login fields came up **empty**. `dev-defaults.ts` gated the ada/acme
prefill on RN's `__DEV__` global, but `vite-plugin-react-native-web` **hardcodes
`const development = false`** and defines `__DEV__: "false"` unconditionally (its
`dist/es/index.js`), regardless of vite mode. A user-config `define: { __DEV__: 'true' }` did not
win the merge. Verified over CDP: `__DEV__ === false`, both text inputs `value === ""`.

**Fix**: stop depending on `__DEV__` for the *web preview*. `dev-defaults.ts` now exposes a mutable
`devLogin` + `setDevLogin(partial)`; the preview entry
[web/index.web.tsx](../../../app/shell/web/index.web.tsx) calls `setDevLogin({user:'ada',
workspace:'acme', nodeUrl})` before `AppRegistry.runApplication` (override via `?user=`/`?ws=`).
The **`__DEV__` gate stays for native** — a real release app still ships empty fields (never a
baked-in identity); the override lives only in preview-only code that never enters a device bundle.

### 3. Pre-existing `localStorage` typecheck errors in the web keychain shim

`keychain.storage.web.ts` (preview-only) used `localStorage` but the RN tsconfig has no `dom` lib →
3 `TS2304` errors (present on clean master, not caused by this session). Fixed with a **file-local**
`/// <reference lib="dom" />` so the web shim types without pulling DOM types into the native build.
`make -C app typecheck` is now clean for both packages.

## The deferred regression test (rule 9 — real gateway, no fakes)

The shell has no component runner yet (RN jest/babel harness is deferred to app-extensions), so the
`client.ts` singleton contract is pinned one layer down, against the **real spawned gateway**:
[app/sdk/tests/client-singleton.gateway.test.ts](../../../app/sdk/tests/client-singleton.gateway.test.ts).

The original bug: the shell rebuilt its `GatewayClient` on node-URL change, swapping out the
`session` store that `useSession` had subscribed to — login updated the new client's store while the
mounted screen listened to the orphaned old one, so the UI never left login. The test asserts the
invariant the rebuild broke: for **one** client instance, a subscriber attached to `client.session`
**before** login fires on login and observes the active session; a second subscriber on the same
client sees it too; `client.session` is the **same object** after login (never replaced); and the
returned unsubscribe actually stops notifications. Real signed token via `client.login(...)`;
memory storage is a storage adapter, not a fake backend (testing §0).

```
$ cd app/sdk && pnpm test:gateway
 ✓ tests/channels.gateway.test.ts (2 tests)
 ✓ tests/client-singleton.gateway.test.ts (1 test)   ← new
 ✓ tests/session.gateway.test.ts (3 tests)
 ✓ tests/caps-deny.gateway.test.ts (4 tests)
 ✓ tests/ext-nav.gateway.test.ts (2 tests)
 ✓ tests/isolation.gateway.test.ts (1 test)
 Test Files  6 passed (6)
      Tests  13 passed (13)
```

`make -C app typecheck` — both packages exit 0.

## End-to-end verification (headless, real gateway)

Drove the live preview over CDP (Node 22's built-in `WebSocket` against the cached Chrome at
`~/.cache/puppeteer` — no puppeteer package is installed, so a zero-dependency CDP driver;
scratch scripts, not committed). `make -C app gateway` (8087) + `pnpm web` (5310), load
`http://127.0.0.1:5310/?node=http://127.0.0.1:8087`, click **Sign in** with the ada/acme prefill:
`document.body.innerText` → `"#acme\nEXT\nWS\nNo channels yet — create one below.\nCreate"` — the
**Channels screen**. Login flows straight through.

- The login POST returns 200 with a real signed token carrying the dev cap set (`bus:chan/*:pub`,
  `mcp:*.list:call`, …) — confirmed directly against 8087.
- **Screenshot caveat (tooling, not the app):** headless-Chrome `captureScreenshot` grabs a blank
  frame for the RN-web tree (RN-web content flexes to 0 measured height under headless), even though
  `innerText` reads the full UI. A real browser on the Linux PC paints it. Functional proof is the
  DOM text + the 13 gateway tests, which is authoritative.
- **Vite gotcha logged:** adding an export to a module that vite already cached (`setDevLogin`)
  serves a stale `?t=` copy over HMR → `does not provide an export named 'setDevLogin'` and a blank
  root. A plain `make web` restart (or clearing `shell/node_modules/.vite`) is clean. Restart vite
  after adding/removing exports; HMR alone won't do it.

## Files touched

- `app/Makefile` — 8087, `/proc/<pid>/exe`-guarded port free, scoped vite kill, dropped store-rm.
- `app/shell/src/lib/dev-defaults.ts` — `DevLogin` type + mutable `devLogin` + `setDevLogin`.
- `app/shell/web/index.web.tsx` — seed the prefill via `setDevLogin` (`?user=`/`?ws=` override).
- `app/shell/src/features/session/keychain.storage.web.ts` — file-local `dom` lib reference.
- `app/sdk/tests/client-singleton.gateway.test.ts` — new singleton regression (rule 9).

## Debugging

No `docs/debugging/` entry — nothing in the node broke; all three causes were preview harness/build
config (a port collision, a plugin's hardcoded `__DEV__`, a missing tsconfig lib). Recorded here at
session level, mirroring the app-shell session's handling of the membership-by-design 403.

## Follow-ups (carried, unchanged from app-shell)

- app-extensions: the RN jest/babel component harness — then the shell-level "login advances the
  UI" component test this session's sdk regression stands in for.
- Real re-mint route to replace re-login on workspace switch when global identity lands one.
