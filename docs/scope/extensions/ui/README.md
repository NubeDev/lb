# Extensions — UI subtopic (`extensions/ui/`)

The extension-facing UI contracts that the **theme customizer** (`frontend/theme-customizer-scope.md`)
forces into existence. When a member (or a workspace admin) changes the theme, *everything* must move
together — including an installed extension's own pages and dashboard widgets — and an extension's
styling must **never** bleed back into the host shell. This subtopic owns those two contracts; the
existing `../ui-federation-scope.md` owns *how* an extension page mounts (module federation vs. iframe,
the host-mediated MCP bridge). This subtopic is what makes a mounted page **look and re-theme native
without leaking**.

## The two scopes

| File | The ask |
|---|---|
| [`theme-inheritance-scope.md`](theme-inheritance-scope.md) | An extension page/widget inherits the host's design tokens and **re-themes live** when the user changes theme — across both trust tiers, including JS/canvas widgets that can't read a CSS variable. |
| [`css-isolation-scope.md`](css-isolation-scope.md) | An extension's CSS **never leaks into the host shell** (the shipped `library-css-leaks-global-utilities` bug, turned into an enforced contract + build guard + runtime fence). |

They are two sides of one boundary — *the host's tokens flow **in**, the extension's styles never flow
**out*** — so they share this index and the token contract below, but split because they have different
owners (theme layer vs. build tooling), different failure modes (stale palette vs. corrupted host), and
different tests (re-theme propagation vs. containment).

## Why now

The shipped `../ui-federation-scope.md` already exposes tokens once at mount and says an extension "styles
within them." Two gaps remain, both made urgent by the customizer:

1. **Mount-time tokens are a snapshot.** The customizer lets the theme change *at runtime* (preset, mode,
   radius, custom colors, workspace default). A page themed only at `mount()` goes stale the moment the
   user picks a new preset. Extensions need a **live** theme signal, not a one-shot.
2. **In-process federation shares the DOM.** A trusted federated remote runs in the host document, so its
   Tailwind/utility CSS can collide with the host's — exactly the shipped bug where `@nube/panel`'s global
   `.flex`/`.border` utilities won the cascade and deleted the app's sidebar. That was a `packages/*`
   library; a federated *extension* remote (e.g. `echarts-panel` ships its own Tailwind build) is the same
   hazard with a third-party publisher. The contract must be **enforced**, not documented.

## The exposed token contract (reference — the canonical surface both scopes share)

The host exposes exactly this set as CSS custom properties on the shell `<html>` (and, for the iframe
tier, injected into the frame document). Extensions consume these; they never hard-code a palette. This is
the frozen list — adding to it is an additive change, removing/renaming one is a breaking change to the
extension surface.

**Base tokens** (the palette the customizer actually writes — the ones every host surface reads):

```
--bg  --panel  --fg  --muted  --muted-foreground  --accent  --border  --radius
```

**Derived shadcn tokens** (aliased from the base tokens in `styles/globals.css`, for extensions built on
shadcn primitives):

```
--background --foreground --card --card-foreground --popover --popover-foreground
--primary --primary-foreground --secondary --secondary-foreground
--accent-foreground --destructive --input --ring
```

Color tokens are **HSL channel triplets** (e.g. `--accent: 34 96% 58%`), consumed as `hsl(var(--accent))`
(the project convention — see `ui/src/features/charts/chartTheme.ts` and `echarts-panel`'s
`tailwind.config.ts`). `--radius` is a length. Light/dark is the `.dark` class on the root; the accent/
preset is inline overrides on the root — an extension that consumes the vars gets both for free.

For JS/canvas consumers that can't use `var(...)`, the same values are delivered **resolved** (concrete
strings) in `ctx.theme` — see `theme-inheritance-scope.md`.

## Build order

1. **`theme-inheritance-scope.md` first** — it lands with the customizer and is what makes the whole
   "everything re-themes" promise true; the shipped in-process tier already inherits DOM tokens, so the
   new work is the live signal + `ctx.theme` for canvas + the iframe injection.
2. **`css-isolation-scope.md` alongside** — the containment fence and build guard protect the in-process
   tier that theme-inheritance leans on; the shipped regression means this is hardening a known hole, not
   speculative.

## Related

- `../ui-federation-scope.md` — how a page mounts (federation/iframe) + the MCP bridge; this subtopic
  themes and fences what it mounts.
- `../ext-sdk-scope.md` — the `lb devkit` build path where the CSS build-guard lands, and the extension
  UI template that ships the correct scoped-CSS skeleton.
- `../../frontend/theme-customizer-scope.md` — the source of the runtime theme changes; emits the signal
  this subtopic propagates.
- `../../frontend/workspace-branding-scope.md` — workspace default theme/branding an extension inherits
  the same way (no special path).
- Memory/precedent: `docs/debugging/frontend/library-css-leaks-global-utilities.md`,
  `docs/debugging/frontend/ce-page-css-preflight-leaks-into-shell.md` — the two real leak bugs the
  isolation scope makes un-repeatable.
- README **§6.13** (extension UIs, design tokens exposed).
</content>
</invoke>
