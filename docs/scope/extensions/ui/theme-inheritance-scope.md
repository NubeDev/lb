# Extensions/UI scope — live theme inheritance

Status: scope (the ask). Part of the `extensions/ui/` subtopic (see `README.md`). Lands with
`frontend/theme-customizer-scope.md`. Promotes to `public/extensions/` once shipped.

An installed extension's page or dashboard widget must **look native and re-theme live** with the host:
when a member changes mode/preset/radius/custom colors — or a workspace admin changes the workspace
default — the extension's UI updates **in place, without a reload**, exactly like the core surfaces do.
The shipped federation scope exposed tokens **once at mount**; the customizer makes the theme a runtime
variable, so a one-shot snapshot goes stale. This scope makes the theme a **live signal** that reaches
both trust tiers, including the hard case: a JS/canvas widget (ECharts) that cannot read a CSS variable
and must be handed **resolved** token values and told to recolor.

## Goals

- **In-process (trusted, federated) DOM consumers re-theme for free.** A federated remote runs in the
  host document; it consumes `hsl(var(--accent))` etc. and inherits every theme change through the CSS
  cascade automatically. This scope's job here is to *guarantee and test* that — not to hard-code fallbacks
  that shadow the host (see `css-isolation-scope.md` rule 2).
- **A live theme signal for JS/canvas consumers.** Widgets that render to a canvas (ECharts, the reference
  `echarts-panel`) can't use `var(...)`; they need **resolved** token values (concrete color strings +
  radius) and a change notification so they re-render in the new palette via the shipped ctx `update`
  hook — no re-mount, no reload.
- **The iframe (untrusted) tier re-themes too.** The sandboxed frame gets the token values injected into
  its own document at mount **and refreshed** on every theme change (a `postMessage` theme event over the
  existing bridge), so an untrusted page tracks the host theme without ever reaching the parent DOM.
- **One theme source, one propagation path.** The theme customizer's theme layer is the single origin;
  `features/ext-host/` is the single fan-out to all mounted extensions. No extension subscribes to
  `localStorage`, reads `document` directly, or polls.
- **`ctx.theme` added to the frozen widget contract** (additive, version-gated) so a widget receives the
  resolved tokens the same way it already receives `ctx.data`/`ctx.fieldConfig`.

## Non-goals

- **No extension override of the host theme.** Tokens flow one way — host → extension. An extension cannot
  restyle the shell chrome or change the host palette (that boundary is `ui-federation-scope.md` Non-goals
  and enforced by `css-isolation-scope.md`).
- **No per-extension theme picker.** An extension inherits the member/workspace theme; it does not ship its
  own theme selector. (Its standalone dev build may use a fallback palette — mount-only.)
- **No new theme *values*.** The token set is the contract in `README.md`; this scope propagates it, it
  doesn't grow it.
- **No SSR/pre-paint theming of remotes.** Remotes are lazy-loaded on nav/slot open; they theme on mount
  and on change, not before they exist.

## Intent / approach

**One emitter, one fan-out, three consumer shapes.**

```
theme customizer changes theme
        │  (member preset/mode/radius/custom  |  admin workspace-default resolved)
        ▼
  theme layer applies base tokens to <html>  ──emits──►  "lb:themechange"
        │                                                      │
        │ (CSS cascade — free)                                 ▼
        ▼                                        features/ext-host  (single subscriber)
  in-process DOM consumers          ┌────────────────────────┼───────────────────────────┐
  (federated remotes using          │ resolve tokens → strings│                           │
   hsl(var(--…))) update            ▼                         ▼                           ▼
   with zero work            federated WIDGET update    federated PAGE (DOM)        untrusted IFRAME
                             (ctx.theme + update())      inherits via cascade;       postMessage
                             ECharts recolors, no        JS-color consumers get      {type:"lb.theme",
                             re-mount                     ctx.theme too              tokens} → frame
                                                                                     refreshes its
                                                                                     injected var block
```

- **Resolve once, hand out strings.** On a theme change (and at mount), `ext-host` reads the *computed*
  values of the exposed tokens off the root (`getComputedStyle`) and builds a `ThemeTokens` object:
  concrete `hsl(...)` color strings for `bg/panel/fg/muted/mutedForeground/accent/border` plus the
  categorical chart ramp (reusing `features/charts/chartTheme.ts` so extension charts match core charts)
  and `radius`. That object is `ctx.theme`, and it is the payload of the iframe `postMessage`.
- **DOM consumers need nothing** beyond consuming the vars — the cascade re-themes them. The scope's value
  for them is the *test* that proves it and the isolation contract that keeps their fallbacks from
  shadowing the host.
- **Canvas/JS consumers** (widgets returning `{ update, teardown }`) get `update(ctx)` called with the new
  `ctx.theme`; the widget rebuilds its ECharts option from the resolved strings and re-renders in place.
- **Iframes** get the initial tokens injected as a `<style>:root{…}</style>` block in the frame document at
  mount, and a `{type:"lb.theme", tokens}` message on change; a tiny frame-side shim replaces the block.
  Origin-checked like every bridge message; the token payload carries no secrets.

**Rejected alternatives:**
- *Leave mount-time tokens as-is (re-mount on theme change).* Rejected — re-mounting every extension on a
  radius nudge is janky, loses widget state, and re-runs federation. The shipped `update` hook exists
  precisely to re-render in place; use it.
- *Let each extension read `localStorage`/`document` for the theme.* Rejected — N subscribers racing the
  theme layer, each re-implementing resolution, and an untrusted iframe *can't* read the parent anyway.
  One emitter, one fan-out.
- *Pass only CSS vars, never resolved strings.* Rejected — it strands every canvas/WebGL widget (ECharts,
  three.js graphics-canvas), which is the whole reason `echarts-panel` exists as the reference. Both: vars
  for DOM, resolved `ctx.theme` for JS.

## How it fits the core

- **Capabilities:** none needed — theme tokens are non-privileged presentation. The theme payload is *not*
  a capability surface and carries no token/secret; it rides the existing page bridge (untrusted) or the
  in-process ctx (trusted). An extension gains no new reach from it.
- **Tenancy / isolation:** the resolved theme reflects the **current session's** resolved preference
  (member → workspace default → built-in, from `prefs`); it is workspace-appropriate by construction
  because the shell already resolved it. No workspace data crosses in the payload — only colors.
- **Symmetric nodes:** identical on browser (hub/SSE) and Tauri; the theme layer and `ext-host` are shell
  code, no role branch.
- **Stateless extensions:** the extension holds no theme state — it re-derives its rendering from the
  `ctx.theme`/vars it is handed on each change. Uninstall/hot-reload stay trivial.
- **MCP is the contract:** unchanged. This is a **presentation** channel (ctx field + a `postMessage`
  event), deliberately *not* an MCP verb — theme is not a tool call. The bridge's MCP surface is untouched.
- **Data / bus:** none. Theme is local UI state; no SurrealDB record, no Zenoh subject added here (the
  *preference* persistence is the customizer's `prefs` story).
- **SDK/WIT impact:** **the widget mount contract gains `ctx.theme`** — an **additive, version-gated** bump
  (`WIDGET_CTX_V` → next), which per the frozen-contract rule (`ext-widget-frames-in-contract`) must move
  in **all three mirrors together**: `ui/src/features/dashboard/builder/federationWidget.ts`, the devkit
  template `rust/crates/devkit/templates/ui/src_contract.ts.tmpl`, and each extension's copy (e.g.
  `echarts-panel`'s `chart/mountChart.ts`). Old (no-`theme`) tiles stay byte-identical — gate on `ctx.v`.
  This is the stop-and-confirm gate for this scope.

## Example flow

1. Alice opens **acme**; her resolved theme is dark + a teal preset. The dashboard mounts an `echarts-panel`
   widget in a cell; the shell resolves the tokens and passes `ctx.theme` (teal accent, dark surfaces) plus
   `ctx.data`. The chart draws in teal-on-charcoal — matching the core charts beside it.
2. She opens the **Customizer** and switches the preset to "Violet Bloom" and radius to `0.75`. The theme
   layer writes the new base tokens to `<html>` and emits `lb:themechange`.
3. Core surfaces re-theme via the cascade. `ext-host` resolves the new tokens and calls the `echarts-panel`
   widget's `update(ctx)` with a fresh `ctx.theme`; the chart **recolors to violet in place**, no re-mount.
4. A federated **page** (DOM, shadcn) beside it re-themes purely through the CSS cascade — nothing to do.
5. A **third-party** page in a sandboxed iframe receives `{type:"lb.theme", tokens:{…violet…}}`; its
   frame-side shim swaps the injected `:root` var block and the page re-themes without touching the parent.
6. A workspace **admin** later sets a workspace-default theme (`prefs.set_default`). A member who never
   customized loads the app; the shell resolves to that default and every extension mounts in it — same
   path, no special case.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` plus this slice's cases. Persistence isolation/
deny is the customizer's; here the categories map as:

- **Live re-theme — canvas widget (`pnpm test`):** mount `echarts-panel` with theme A; emit a theme change
  to theme B; assert `update(ctx)` fires with the new resolved `ctx.theme` and the ECharts option reflects
  B's accent — **no re-mount** (the mount fn called once).
- **Live re-theme — DOM remote:** a federated DOM component consuming `hsl(var(--accent))` reflects the new
  accent after a change with no explicit update (cascade), proving in-process inheritance.
- **Live re-theme — iframe:** on theme change the frame receives one `{type:"lb.theme", tokens}` message and
  swaps its injected var block; assert the frame document's `:root` vars updated and the **token payload
  contains no session token/secret** (only colors + radius).
- **Workspace isolation (mandatory):** the resolved `ctx.theme` a page receives comes from the **session's**
  resolved prefs; a ws-B session's extensions get ws-B's resolved theme; no cross-workspace value leaks in
  the payload (two real sessions).
- **Capability deny (mandatory):** the theme channel grants **no** new reach — assert an extension cannot
  use the theme event/`ctx.theme` to call a tool or read anything beyond its granted bridge scope (the
  deny surface is unchanged by adding theme).
- **Contract version gate:** a v-prev widget (no `ctx.theme`) still mounts and renders byte-identically
  (gate on `ctx.v`); a v-next widget receives `ctx.theme`. Test all three mirrors stay in lockstep.
- **Gateway/e2e (`pnpm test:gateway`):** against a real node, change the theme in the shell and observe a
  mounted reference extension (`echarts-panel` and a federated page) re-theme live — no fake backend.

## Decisions (resolved — no open questions)

- **Change signal = a shell-internal `lb:themechange` event** emitted by the theme layer (a small
  pub/sub in `lib/theme`, not a DOM `CustomEvent` global and not `localStorage`), with `features/ext-host`
  the **only** subscriber that fans out to extensions. Rationale: one testable seam, no N-subscriber race,
  untrusted iframes can't read the parent so a shell-mediated fan-out is mandatory anyway.
- **Token delivery = both forms.** DOM consumers use the CSS vars on the root (in-process) / injected block
  (iframe); JS/canvas consumers get **resolved strings** in `ctx.theme`. Not one or the other — each shape
  needs its own.
- **`ctx.theme` shape** = `{ bg, panel, fg, muted, mutedForeground, accent, border, radius, chart: string[] }`,
  all concrete strings, `chart` reusing `chartTheme.ts`'s categorical ramp so extension charts match core
  charts. Resolved from `getComputedStyle` on change, so custom/imported colors are honored, not just presets.
- **Re-render, never re-mount.** Theme changes drive the shipped `update(ctx)` hook; a widget without an
  `update` (bare v2 tile) is left as-is (DOM tiles re-theme via cascade regardless). No forced remount.
- **iframe theme message** = `{type:"lb.theme", tokens:<ThemeTokens>}`, origin-checked, additive to the
  existing bridge protocol; the frame ships a ~10-line shim (in the devkit iframe template) to apply it.
- **Contract bump is version-gated and confirmed as the SDK gate.** `WIDGET_CTX_V` increments; all three
  mirrors move together; old tiles gated on `ctx.v`. This is the one stop-and-confirm before landing.
- **No new capability, no MCP verb, no store/bus record** for theme. Presentation channel only. Settled.

## Related

- `README.md` (this subtopic) — the exposed token contract `ctx.theme` resolves from.
- `css-isolation-scope.md` — the sibling that keeps an extension's fallback palette/utilities from
  shadowing or leaking into the host (rule 2 is why in-process inheritance works at all).
- `../ui-federation-scope.md` — the mount + bridge this rides on; extends its one-shot token exposure to a
  live signal.
- `../../frontend/theme-customizer-scope.md` — the emitter; the `lb:themechange` origin and the resolved
  member→workspace-default preference.
- `ext-widget-frames-in-contract` (memory) + `ui/src/features/dashboard/builder/federationWidget.ts` — the
  frozen `ctx` contract `ctx.theme` extends, and the three mirrors that must move together.
- `rust/extensions/echarts-panel` — the reference canvas widget that proves the `ctx.theme` + `update` path.
- `ui/src/features/charts/chartTheme.ts` — the categorical ramp reused so extension charts match core.
</content>
</invoke>
