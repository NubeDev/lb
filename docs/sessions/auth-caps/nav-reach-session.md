# Session — nav gates reach (a curated nav is the allow-list of reachable pages)

Branch: `update-auth`. Scope: `docs/scope/auth-caps/nav-reach-scope.md`. Builds on the shipped `viewer`
role (`login-hardening-scope.md`).

## The ask

`user:bob`, a `viewer` given a nav of exactly ONE page, could still OPEN every other read-only page
(Rules/Flows/Ingest/Datasources/Data Studio) by URL — they rendered read-only. The `viewer` role made
him read-only (a coarse tier) but not "one page." The user chose (explicitly, over the prior
"nav is a pure lens, never gates" rule) to make **the nav the allow-list of reachable core pages, read
included, enforced server-side**. One page in the nav ⇒ every other page denied.

## What shipped

**A new `reach:<surface>:view` capability, derived from the resolved nav, enforced by a dedicated
page-entry route.** Reach is now gated by a cap like everything else — it rides the one `lb_caps::check`
choke point, so there is no new gate to wire into (and forget on) every route.

1. **Grammar surface `reach` + action `view`** — `rust/crates/caps/src/request.rs`. The comment there
   already sanctioned this ("a new surface is a deliberate grammar change"). `reach:rules:view`,
   `reach:*:view`. Tests: `crates/caps/tests/match_test.rs` (concrete gates one page; wildcard reaches
   all).
2. **Nav → reach caps** — `rust/crates/host/src/nav/reach.rs` (`reach_caps` + `reach_check`). A curated
   nav yields one `reach:<surface>:view` per menu surface (surface/dashboard/group items; `ext` ignored
   — ext reach stays the `ext.list` seam, rule 10). **A fallback nav yields the wildcard `reach:*:view`**
   (reaches all — the no-lock-out guard). `reach_check` degrades OPEN when a token carries NO `reach:`
   cap at all (legacy/API-key tokens) — deny only when reach data is PRESENT and says no.
3. **Login fold** — `rust/role/gateway/src/routes/login.rs`. After the grant fold, resolve the subject's
   nav under their full caps and union `reach_caps(resolved)` into the token. Degrades open on a
   resolve error (`reach:*:view`), never fails login.
4. **The server boundary** — `GET /surface/{surface}` (`rust/role/gateway/src/routes/surface.rs` +
   `session/reach.rs::require_reach`). Returns 200 iff the caller holds `reach:<surface>:view` (or the
   wildcard), else opaque 403. A gated page's client loader awaits this; the SHARED data routes stay
   open. `{surface}` is opaque (rule 10).
5. **Client guard (UX + defense-in-depth)** — `ui/src/features/routing/allowed.ts`. `allowedSurfaces`
   now intersects the cap-allowed set with the nav-derived reach caps already in the token
   (`mayReachSurface`), so the rail drops non-nav pages AND `CoreGate` redirects a deep link. Degrades
   open on absence, mirroring the server. Tests: `ui/src/features/routing/allowed.test.ts`.

## The hard part (what the design turned on)

**The list routes are NOT page-exclusive.** The pages to block (Ingest/Rules/Flows/Datasources) load
via `GET /series`, `/rules`, `/flows`, `/datasources` — the SAME routes the dashboard source picker,
Data Studio panel builder, nav editor, and channel pin picker call (`ui/.../builder/useSourcePicker.ts`
fans them into every tile render). So the server **cannot distinguish** "open the Ingest page" from
"a dashboard tile lists series" — same HTTP request. Gating those would break the pages bob IS allowed
to see. An `X-LB-Surface` header is not a boundary either (bob curls any value).

→ The user chose the RIGHT fix: a **dedicated per-page entry route** (`GET /surface/{s}`) distinct from
every shared data route. The data routes stay open (tiles/pickers work); *opening a page* is the hard
server boundary. This is why reach is a page-entry gate, not a per-verb gate.

## Tests (all green, real gateway + SurrealDB, no mocks — CLAUDE §9)

- `cargo test -p lb-caps --test match_test` — grammar: reach gates one page, wildcard reaches all.
- `cargo test -p lb-host --lib nav::reach` (6) — derivation (fallback→wildcard, curated→one surface,
  dashboard→dashboards, group recurse, pins, ext ignored) + `reach_check` (concrete deny + degrade-open).
- `cargo test -p lb-role-gateway --test nav_reach_test -- --test-threads=1` (2, the headline):
  - curated one-page nav ⇒ `GET /surface/dashboards` 200 but `/surface/{ingest,rules,flows,datasources,
    telemetry,system}` all **403**; the SHARED `GET /series` still **200** (tile data unaffected); a
    fallback admin 200s every `/surface/{s}` (no lock-out).
  - workspace-walled: an acme workspace-default curated nav never affects a beta subject (reaches all).
- `ui vitest allowed.test.ts` (5) + `App.test.tsx` (3, unchanged, still green after the reach filter).
- Regression-clean: `viewer_reach_test`, `login_hardening_test`, `gateway_test`, `mcp_bridge_test`,
  host lib `nav::*`. `cargo fmt` + `cargo build --workspace` green.

## Docs updated

- `docs/scope/auth-caps/nav-reach-scope.md` — the scope (overturns "pure lens" in the narrowing
  direction, with the dedicated-route rationale).
- `docs/scope/auth-caps/access-model-scope.md` + `docs/skills/nav/SKILL.md` — amended the "pure lens,
  never gates" rule to "lens for widening, gates reach for narrowing."

## Follow-ups (not blocking)

- Reach refreshes on **re-login** (like every cap). A nav edit reaches a logged-in user on their next
  login. A future token-refresh could re-fold reach; out of scope here.
- The `GET /surface/{s}` route is available for a live client `beforeLoad` preflight; the current client
  uses the token's reach caps directly (synchronous, no fetch). Either is fine — the route is the
  boundary regardless.
