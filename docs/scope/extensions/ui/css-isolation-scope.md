# Extensions/UI scope — CSS isolation (no leakage into the host shell)

Status: scope (the ask). Part of the `extensions/ui/` subtopic (see `README.md`). Hardens the shipped
in-process federation tier. Promotes to `public/extensions/` once shipped.

A trusted, module-federated extension renders **in the host document**, so its stylesheet shares the
cascade with the shell. If that stylesheet ships global utilities, a preflight/reset, or a fixed palette,
it **corrupts the host** — this has already happened: `@nube/panel`'s top-level `tailwindcss/utilities.css`
emitted global `.flex`/`.border`/… rules that beat the app's own utilities and **deleted the left
sidebar**, and its fixed palette rendered the selected nav item as a black block in light mode
(`docs/debugging/frontend/library-css-leaks-global-utilities.md`, resolved 2026-07-02; a sibling preflight
leak in `ce-page-css-preflight-leaks-into-shell.md`). Those were `packages/*` libraries; a federated
**extension** remote (e.g. `echarts-panel` ships its own Tailwind build) is the identical hazard with a
third-party publisher and no code review. This scope turns the ad-hoc "three rules" fix into an
**enforced contract**: authored scoping, a **build-time guard** that fails the build, and a **runtime
fence** so even a non-conforming remote cannot beat the host. The theme must flow *in* (see
`theme-inheritance-scope.md`); the extension's styles must never flow *out*.

## Goals

- **A written remote-CSS contract** every federated extension UI must satisfy, generalized from the
  shipped fix: (1) utilities **scoped** under the extension's root class — never global; (2) theme tokens
  **aliased** onto the host vars (`--lbx-…: var(--card, <fallback>)`), fallback used only for standalone
  dev, so the host's live theme always wins; (3) **no preflight / no `@layer base`** — a remote must not
  reset its host.
- **A build-time guard** in the extension toolchain (`lb devkit` / the UI template's `build.sh`) that
  inspects the **built** remote CSS and **fails the build** on a violation (global utility rules, a
  `@layer base`, or an unscoped selector), so a leak can't be published — the same `grep` assertions the
  debug fix hand-checked, now automated.
- **A runtime containment fence** in `features/ext-host/`: the shell mounts every federated remote inside a
  container and places extension styles in a **lower cascade layer**, so a host utility always outranks an
  extension utility of the same name even if the guard is somehow bypassed (defense in depth).
- **The devkit UI template ships correct-by-construction** — the generated extension scaffolds with scoped
  utilities, aliased tokens, and no preflight, so a new extension starts compliant and the guard is a
  backstop, not the teacher.
- **Regression coverage** that reproduces the shipped failure (a remote with global `.flex` + a preflight)
  and proves the guard rejects it and the fence neutralizes it.

## Non-goals

- **The untrusted iframe tier needs none of this.** A sandboxed frame is a separate document — its CSS
  physically cannot reach the host. Isolation there is the sandbox itself; the frame only *receives*
  injected theme vars (`theme-inheritance-scope.md`). This scope is about the **in-process** tier.
- **No Shadow DOM rewrite of the mount host.** Considered and rejected below; the layered-cascade + build
  guard is lighter and preserves token inheritance. (Kept as a fallback lever, not the plan.)
- **No restyling of extensions by the host** beyond exposing tokens. The host contains and themes; it does
  not reach into an extension's internals.
- **No change to the federation/bridge/trust model** — that is `ui-federation-scope.md`. This adds a CSS
  contract + guard + fence on top of the shipped mount.

## Intent / approach

**Contain at three layers, so any single failure is caught by the next.**

1. **Authored scoping (prevention).** The extension's stylesheet uses the v4 nesting form so utilities are
   emitted *under the extension root class*, never globally:
   ```css
   @layer ext-utilities { .lbx-<id> { @tailwind utilities } }
   ```
   Tokens are aliased, not fixed: `--lbx-panel: var(--card, <darkfallback>)`; the fallback only applies
   when the remote is mounted standalone (dev), so inside the shell the host theme wins — which is *also*
   what makes `theme-inheritance-scope.md` work. No `@layer base` / preflight.
2. **Build-time guard (enforcement).** `lb devkit build` (and the template `build.sh`) runs assertions on
   the built CSS and **fails** on any of: a global utility rule (`grep -oE '}\.(flex|grid|border|hidden|w-full|…)\{'` non-empty),
   a `@layer base` present (`grep -c '@layer base'` > 0), or a top-level selector not under the ext root
   class. This is the manual check from the debug entry, promoted to a gate — a leak cannot be published.
3. **Runtime fence (containment).** `features/ext-host` mounts a federated remote inside a
   `<div class="lbx-<id>">` and registers the remote's styles in a **named cascade layer below the host's**
   (`@layer host, ext;` — `ext` loses ties). So even a non-conforming global `.flex` from a remote is
   outranked by the host's `.flex`. The fence is not an excuse to skip the guard (a lower layer still
   *exists* globally and could match unintended nodes) — it's the safety net.

**Why layered cascade over Shadow DOM.** Shadow DOM gives hard isolation but **breaks token inheritance
via the cascade** (custom properties do inherit into a shadow tree, but component libraries, portals, and
the shipped federation `mount(el,…)` contract assume light-DOM), and would fork the whole federation path.
The layered-cascade + scoped-build approach keeps the shipped in-process contract intact, preserves the
free token inheritance `theme-inheritance-scope.md` relies on, and still yields "host always wins."
Shadow DOM stays documented as the escape hatch if a future untrusted-but-in-process case ever appears
(it shouldn't — untrusted always iframes).

**Rejected alternatives:**
- *Documentation only (the current state).* Rejected — it already failed once in production; a memory note
  and a debug entry did not stop `@nube/panel` from shipping the leak. Enforcement is the point.
- *Iframe everything to force isolation.* Rejected — first-party pages must feel native and in-process
  (the whole trusted tier). Isolation for in-process is a build+cascade problem, not a sandbox problem.
- *Strip/rewrite extension CSS at load in the shell.* Rejected — fragile, opaque, and it would also strip
  legitimate scoped styles; catch it at **build** where the author can fix it, plus a cheap runtime layer.

## How it fits the core

- **Capabilities:** N/A — CSS containment is a build/runtime hygiene boundary, not a privileged action.
  (It *protects* the shell's integrity, which underpins every capability-gated surface rendering correctly.)
- **Tenancy / isolation:** indirect but real — a corrupted shell (vanished nav, unreadable controls) is a
  reliability/availability hole across every workspace. Containing extension CSS keeps the shared shell
  trustworthy for all tenants. No workspace data is involved.
- **Symmetric nodes:** the build guard runs wherever extensions are built (`lb devkit`, local-only per the
  toolchain scope); the runtime fence is shell code, identical on browser and Tauri. No role branch.
- **Core knows no extension (rule 10):** the fence keys off the **opaque** extension id (`lbx-<id>` /
  the install's ui decl) — it does not branch on *which* extension. Every federated remote is contained
  identically; a built-in gets no CSS pass that a third-party lacks.
- **Stateless extensions:** unaffected — this is styling hygiene, no instance state.
- **No mocks / no fake backend:** the guard is tested against **real** built CSS (a real bad remote and a
  real good one); the fence is tested by mounting a real federated remote in a real shell and asserting the
  host survives. No fake.
- **SDK/WIT impact:** the **remote-CSS contract is a published extension-authoring contract** (like the
  manifest) — additive, and the devkit template encodes it. Not a WIT/ABI change; note it in the SDK docs so
  authors know the guard's rules. No stop-and-confirm ABI gate (it constrains CSS, not the binary boundary).

## Example flow

1. A dev scaffolds a UI extension via `lb devkit`; the template's `styles/` already scopes utilities under
   `.lbx-<id>`, aliases tokens to host vars, and ships no preflight — compliant by construction.
2. They accidentally add a top-level `@import "tailwindcss/utilities.css"` for convenience. `lb devkit
   build` runs the CSS guard, finds global `}.flex{` rules, and **fails the build** with a pointer to the
   contract and the offending selectors. Nothing ships.
3. They fix it (nest under the root class); the build passes; the signed remote publishes.
4. The shell installs it and opens its slot. `ext-host` mounts the remote inside `<div class="lbx-…">` with
   its styles in the `ext` cascade layer (below `host`).
5. A member switches to light mode. The remote re-themes because its tokens alias the host vars
   (`theme-inheritance-scope.md`); the host sidebar and nav are untouched — no black block, no vanished
   sidebar. The 2026-07-02 failure is now impossible to ship *and* neutralized if it somehow mounted.

## Testing plan

Mandatory categories map to hygiene/containment here; plus this slice's cases.

- **Build guard — rejects a leak (`cargo test` / devkit test):** feed the guard a built CSS containing a
  global `.flex` rule, a `@layer base`, and an unscoped top-level selector; assert each **fails the build**
  with an actionable message. Feed it the compliant template output; assert it **passes**. (This is the
  shipped debug entry's manual `grep` checks, now an automated regression.)
- **Runtime fence — host survives a bad remote (`pnpm test:gateway`, real shell):** mount a federated
  remote that ships a global `.flex` + a preflight; assert the **host sidebar and nav still render** and
  the host's own utilities win the cascade (reproduce the exact 2026-07-02 symptom, prove it no longer
  breaks the shell). This must run in the **real app path**, not jsdom — the original bug was
  jsdom-invisible (memory `ui-library-css-rules`), so the regression test uses the gateway/real-DOM harness.
- **Token aliasing — no shadow (`pnpm test`):** a remote using `--lbx-*: var(--card,…)` reflects the host
  theme (not its fallback) when mounted in the shell; the fallback shows only standalone.
- **Contract conformance — the shipped reference extensions** (`echarts-panel`, `fleet-monitor`) pass the
  guard; wire the guard into their `build.sh` so a future regression in a reference ext is caught in CI.
- **Isolation direction:** assert the containment is one-way — the host may theme the extension (tokens in),
  but no extension selector modifies a host node outside its container (styles out = blocked).

## Decisions (resolved — no open questions)

- **Contain with a layered cascade + container class, not Shadow DOM.** `@layer host, ext;`, remote styles
  in `ext`, mounted under `.lbx-<id>`. Preserves token inheritance and the shipped `mount(el,…)` contract;
  Shadow DOM kept only as a documented escape hatch that is not planned.
- **The build guard lives in `lb devkit build` and the UI template `build.sh`, and fails the build.** Not a
  lint-warning, not a runtime console message — a hard build failure, because the failure mode is a broken
  production shell. Same assertions the debug fix used, automated.
- **The guard's rule set is fixed as:** (a) no global utility rules (scoped-under-root only), (b) no
  `@layer base`/preflight, (c) no top-level selector outside the ext root class. Adding a rule is additive;
  these three are the mandatory floor (they are exactly the two shipped leak classes plus the palette rule).
- **Fallback palette is allowed but only via `var(--host, <fallback>)`** so it can never shadow the host in
  the shell — a bare fixed palette is a guard failure. This is the same aliasing that makes live
  inheritance work, so the two scopes reinforce each other.
- **Untrusted/iframe is out of scope by construction** — sandbox isolates it; it only receives injected
  theme vars. No CSS contract applies across a frame boundary.
- **Keying is by opaque `lbx-<id>`**, never by a named extension — rule 10 holds; every remote is fenced
  identically.

## Related

- `docs/debugging/frontend/library-css-leaks-global-utilities.md` + `ce-page-css-preflight-leaks-into-shell.md`
  — the two shipped leak bugs this scope makes un-shippable; the manual `grep` checks that become the guard.
- `ui-library-css-rules` (memory) — the three rules generalized here into an enforced contract.
- `theme-inheritance-scope.md` — the sibling: tokens flow **in** (aliasing is shared with rule 2); this
  scope stops styles flowing **out**.
- `../ui-federation-scope.md` — the in-process mount this fences; the trusted tier that shares the DOM.
- `../ext-sdk-scope.md` — `lb devkit build` (where the guard lands) and the UI template (correct-by-
  construction scaffold).
- `rust/extensions/echarts-panel`, `rust/extensions/fleet-monitor` — reference remotes wired to the guard.
- README **§6.13** (extension UIs / design tokens); Non-negotiable **rule 10** (core keys off opaque ids).
</content>
</invoke>
