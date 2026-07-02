# Slice 9 — federated page CSS isolation (no host reset, host-themed)

Status: scope slice (S9, hardening). Depends on: S7 (the federated `[ui]` page).
Parent: `control-engine-scope.md`. Sibling precedent:
`docs/debugging/frontend/library-css-leaks-global-utilities.md` (the SAME class of bug,
fixed once for the workspace `packages/*` — this slice closes it for the standalone
extension UIs, which that fix did not touch).

## The ask

The control-engine federated page, once mounted, **took over the whole shell** — the
main nav + sidebar collapsed and the page filled the viewport. Root cause is CSS, not
React: the extension injects a stylesheet that carries Tailwind's global reset
(Preflight) into the **host** document.

Two requirements, long-term, no hacks:

1. **A federated page must never mutate the host's global styles.** It ships only its
   own scoped rules; the shell's `html/body/*` reset, layout, and utilities are the
   host's and are never re-declared by an extension.
2. **A federated page should look native** — it follows the host's light/dark theme and
   accent automatically, without mirroring a private copy of the palette that drifts.

## Root cause (measured)

`ui/src/remoteEntry.ts` injects two `<style>` blocks into `document.head` on first
mount:

- `styles` — the page's own compiled `src/styles/tokens.css`, whose first three lines are
  `@tailwind base; @tailwind components; @tailwind utilities;`.
- `editorStyles` — the vendored `@nube/ce-wiresheet` bundled theme (injected `?raw`,
  already-compiled; a separate concern, see "The editor stylesheet" below).

`@tailwind base` expands to **Preflight** — a *global* reset targeting
`*, ::before, ::after, html, body, h1–h6, button, input, …` (`box-sizing:border-box`,
`margin:0`, `-webkit-text-size-adjust`, unstyled headings/lists, …). Verified present in
the built bundle:

```
$ grep -oE '\*,::before,::after|body\{|margin:0|-webkit-text-size-adjust' dist/remoteEntry-*.js
  *,::before,::after   box-sizing:border-box
  body{ …
  margin:0  (×8)
  -webkit-text-size-adjust
```

Injected into the shell's `<head>` **after** the shell's own `globals.css`, later-same-
specificity rules win the cascade, so the extension's Preflight re-resets the live shell:
`body`/`*` margins and box-sizing change under the running layout, the shell's flex
sidebar collapses, and the page spills to full width. This is exactly
`library-css-leaks-global-utilities.md`'s finding — a **library/federated** stylesheet
must not ship Preflight.

Secondary drift: `tokens.css` **re-declares** `:root { --bg … --accent … }` with a fixed
amber and **no `[data-theme-accent]` support**, so even without the reset the page ignores
the host's teal/blue accent and any future token change — a second, quieter form of the
same "don't own the host's globals" violation.

> **Scope note — this is not CE-specific.** `proof-panel` (the reference UI extension)
> ships the identical `@tailwind base` + re-declared `:root` in its own `tokens.css`. It
> is less visible only because its page is smaller. The contract below is written to be
> **the rule for every extension UI**; CE is the driving case. Fixing CE without writing
> the contract would let the next extension re-introduce it.

## Decision — the federated-page CSS contract

An extension's injected stylesheet is a **library** stylesheet, not an app stylesheet.
Three rules, each closing one leak:

### 1. No Preflight. Ever.

Drop `@tailwind base;` from the extension entry stylesheet. The host already owns the
reset (`ui/src/styles/globals.css`), and every federated page renders **in-process against
the host document** (S7: `ExtHost` calls `mount(el, …)` into the shell's DOM), so the host
reset already applies to the page. An extension shipping its own Preflight can only
*fight* the host's — never add anything the page needs.

### 2. Scope every generated utility under the page root.

`@tailwind components` / `@tailwind utilities` emit **global** class rules (`.flex`,
`.border`, `.rounded-md`, …). Two independently-built Tailwinds (the shell's v4 and the
extension's v3) emitting the same class names into one document is a cascade collision
(the `library-css-leaks-global-utilities` fix hit exactly this across a v4.1↔v4.3 minor).
Scope them under a stable page-root class so they can only match inside the extension's
own subtree:

```css
/* tokens.css — the extension entry stylesheet (Tailwind v3, PostCSS) */
@layer components { .ce-page { @tailwind components; } }
@layer utilities  { .ce-page { @tailwind utilities;  } }
/* NO @tailwind base */
```

This emits `.ce-page .flex { … }` instead of `.flex { … }` — the extension's utilities
are inert outside its root and cannot touch the shell. The page's own root element carries
the class:

```tsx
// Page.tsx — the single mount root the shell hands us
<div className="ce-page flex h-full flex-col …" data-control-engine-page> … </div>
```

(One class on the outermost element the extension renders; every descendant is covered.
`ExtHost` mounts into `el`, and `Page` is the only child, so one `.ce-page` at `Page`'s
root is sufficient and complete.)

### 3. Inherit host tokens; keep a fallback only for standalone dev.

The shell defines the palette on `:root` (`--bg --panel --border --fg --muted --accent`
plus the shadcn `--background/--card/…` aliases) and theme-swaps them via `.dark` +
`[data-theme-accent]` (`ui/src/styles/globals.css`). Because the page renders in the host
document, **those variables are already in scope** — the extension should simply *use*
them (`bg-bg`, `text-fg`, `border-border`, `text-accent` resolve against the host's live
values) and **not re-declare `:root`**. The page then tracks the host's light/dark +
accent for free, with zero mirrored palette to drift.

Keep a **fallback** block for when the page is served **standalone** (dev `vite`, no host
`:root`), scoped to the page root so it never overrides the host:

```css
/* Fallback ONLY — host :root wins when present (its vars are set on a higher element).
   Scoped to .ce-page so a standalone dev server still has a palette, but this never
   redefines or overrides the shell's tokens. */
.ce-page {
  --bg: var(--bg, 40 30% 96%);
  --panel: var(--panel, 40 24% 92%);
  --border: var(--border, 40 12% 82%);
  --fg: var(--fg, 30 12% 14%);
  --muted: var(--muted, 30 6% 44%);
  --accent: var(--accent, 32 92% 34%);
}
```

(When the host `:root` sets `--bg`, the page's Tailwind color `hsl(var(--bg))` resolves it
from `:root` — the `.ce-page` fallback's `var(--bg, …)` self-reference only supplies the
default when no ancestor defines it. Net: host present → host wins; standalone → fallback.)

### Rejected alternatives (why not)

- **Import the host's compiled CSS into the remote.** A federated remote is a *separate*
  bundle built at a different time against a different Tailwind (v3 here, v4 in the shell);
  it cannot import the shell's compiled utilities, and pinning to them couples build
  versions. Inheriting *tokens* (runtime CSS vars) gives the native look without coupling
  *builds*. Rejected.
- **Shadow DOM around the page.** True isolation, but it also walls the page off from the
  host's tokens and fonts (defeating "look native"), breaks the vendored editor's
  portal-based menus/tooltips that expect the light DOM, and is a large change for a
  problem the scope-class already solves. Rejected for v1; revisit only if a future
  untrusted-iframe tier needs it (that tier is already the isolation story — see S7's
  iframe-sandbox follow-up).
- **Keep Preflight but inject it into `el` instead of `<head>`.** `<style>` in a subtree
  still applies its `*`/`body` selectors document-wide (CSS has no "scope a raw Preflight
  to a subtree" without rewriting its selectors — which is rule 2 anyway). Rejected.

## The editor stylesheet (`@nube/ce-wiresheet/style.css`)

The vendored editor's theme is injected `?raw` (verbatim, already compiled as Tailwind
**v4**). It is a *separate* artifact from the page's `tokens.css` and must obey the same
contract at the point it is BUILT (in `packages/ce-wiresheet`, the `build:lib` output):

- It must not carry Preflight into the host either. Confirm the ce-wiresheet lib build
  emits **no** `*{}`/`body{}` reset into `dist/ce-wiresheet.css` (audit the built file the
  same `grep` way). If it does, the fix is upstream in `packages/ce-wiresheet`'s CSS entry
  (scope + drop preflight there), then re-vendor — the S2 rule (editor fixes go upstream,
  never patched in the extension).
- Its own class rules should already be editor-namespaced (xyflow/codemirror class
  prefixes); if it emits bare Tailwind utilities, scope them under the editor root the same
  way. **Measure the built file; record what it actually emits before changing it.**

This slice's REQUIRED audit step: `grep` both built stylesheets
(`control-engine/ui/dist/remoteEntry-*.js` and `packages/ce-wiresheet/dist/ce-wiresheet.css`)
for `*,::before`/`body{`/`-webkit-text-size-adjust` and record the counts before/after.

## Files touched

- `rust/extensions/control-engine/ui/src/styles/tokens.css` — drop `@tailwind base`;
  scope `components`/`utilities` under `.ce-page`; replace the `:root` re-declaration with
  a `.ce-page` fallback block. (Tailwind v3 `@layer` nesting via the existing PostCSS
  pipeline — no new tooling.)
- `rust/extensions/control-engine/ui/src/Page.tsx` — add `ce-page` to the page-root
  element's className (it already has `data-control-engine-page`).
- `packages/ce-wiresheet` (only IF the audit shows its built CSS leaks Preflight) — scope +
  drop preflight in the lib CSS entry, re-run `build:lib`, re-vendor. Upstream, per S2.

No Rust, no verb, no route, no capability change — this is entirely the page's own styling.

## Tests (rule 9 — real, no mocks)

1. **Built-artifact audit (regression guard).** A test (or a `build.sh` post-step) asserts
   the built `remoteEntry.js` contains **zero** Preflight signatures
   (`*,::before,::after`, `-webkit-text-size-adjust`, a bare `body{`). This is the durable
   guard — it fails the build if `@tailwind base` ever returns. Same check for
   `packages/ce-wiresheet/dist/ce-wiresheet.css`.
2. **Scoped-utility unit test.** Compile `tokens.css` and assert every utility rule is
   prefixed with `.ce-page ` (no bare `.flex`/`.border` at rule root) — mirrors the
   nav-rail/panel scoped-utility assertions from the `library-css-leaks-global-utilities`
   fix.
3. **Live shell check (the real proof).** Under `make dev CE=1`, open Control Engine: the
   host nav + sidebar stay intact, the page occupies only its route surface, and toggling
   the host light/dark + accent theme re-themes the CE page chrome (proving token
   inheritance). Record the before/after in the session doc — the running shell is the only
   thing that revealed the bug (jsdom unit tests never do; see the debugging entry).

## Exit gate

- `tokens.css` ships no Preflight and no `:root` re-declaration; utilities are
  `.ce-page`-scoped; the page root carries `.ce-page`.
- Both built stylesheets audit clean (zero Preflight signatures).
- Live: opening CE does not disturb the shell layout, and the CE page follows the host
  theme/accent.
- A debugging entry records the root cause + the contract, and
  `docs/debugging/README.md` is updated.

## Related

- `docs/debugging/frontend/library-css-leaks-global-utilities.md` — the first instance of
  this exact class (workspace `packages/*`); this slice applies the same three-part fix to
  the standalone extension UIs it didn't cover.
- `ui/src/styles/globals.css` — the host's token + reset source of truth (the vars this
  slice inherits).
- `slice-7-bridge-transport-ui.md` — the federated page this hardens.
- README §3 (symmetric nodes / no private forks of shared truth), FILE-LAYOUT (one
  responsibility per file — the entry stylesheet is styling only).
