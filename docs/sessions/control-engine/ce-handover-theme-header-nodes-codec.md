# Handover — CE extension: theme, header clash, node slots, and the `source_uid` codec crash

- **Date opened:** 2026-07-03
- **Branch:** `ce-node-wiring-v2` (STAY on it)
- **Context:** The federated CSS leaks are fixed (slice-9 + 9.1 — see
  [ce-slice-9-federated-css-isolation-session.md](ce-slice-9-federated-css-isolation-session.md) and
  [../../debugging/frontend/ce-page-css-preflight-leaks-into-shell.md](../../debugging/frontend/ce-page-css-preflight-leaks-into-shell.md)).
  This handover covers the NEXT set of live issues seen when opening Control Engine (screenshot: canvas
  blank, a red error toast, doubled header). **Four issues, two CSS/UI + two CE-package/data.**

The blank canvas in the screenshot is caused by issue **#4** (the codec crash returns a bad host response,
so the editor has no graph to render). Fix #4 first — it's the one that makes the page unusable.

## Progress (2026-07-03)

- **#4 codec crash — ✅ FIXED.** Extension-side tolerant raw fetch (`tools::raw_tree`) bypasses
  `rubix-ce`'s strict `EdgeDto` decode; `serve.rs` routes the real `control-engine.tree` there. Rebuilt
  `-p control-engine` (green) + rule-9 regression (23 lib tests pass). See
  [../../debugging/frontend/ce-tree-missing-source-uid-blanks-canvas.md](../../debugging/frontend/ce-tree-missing-source-uid-blanks-canvas.md).
- **#1 theme — ✅ FIXED.** `useDocumentColorMode` now reads `.dark` (host convention) instead of the
  never-set `.light`. Rebuilt `build:lib` + ext UI (green).
- **#3 node slots / streaming — ✅ FIXED (was downstream of #4).** Confirmed against the live ce-studio
  engine: the decode crash was the sole cause — no graph decoded → no children/ports/edges. With the
  tolerant fetch the wiresheet gets the raw camelCase graph (`properties` → ports;
  `sourceUid`/`targetUid` + `sourceProperty`/`targetProperty` → wires). Proven by the live
  `bridge-transport.live.test.ts` (spawns the real sidecar vs the running engine, asserts real camelCase
  edges through the full bridge; 4/4).
- **#2 header clash — ✅ FIXED.** Removed the CE page's own `<header>` (`Page.tsx`) — the host header owns
  the title + workspace. The appliance **picker** now renders only when there's a real choice (≥2
  appliances); the connected/disconnected badge already lives inside `CeEditor` (`ConnectionStatus`), so it
  survives. `mount.test.tsx` updated (no "Control Engine" title, no single-appliance picker; a 2-appliance
  test proves the picker). ext UI suite 36 passed, build clean.

### Live-engine root cause (important — the handover's #4 hypothesis was half-right)

Capturing the real payload (`curl 127.0.0.1:7979/api/v0/nodes?depth=-1&withEdges=true`) showed the engine
emits **camelCase** keys (`sourceUid`, `childrenCount`, `sourcePropertyUid`, …). The crate's `EdgeDto` is
snake_case with no `rename_all`, so it wasn't ONE dangling edge — **every** edge failed the decode. The
wiresheet's own `engine-types.ts`/`rfbuild.ts` read the camelCase shape directly, so the tolerant raw
pass-through is exactly what it wants. Full detail in the debugging entry.

**Still to do (operational, not code):** republish the rebuilt `control-engine` sidecar to the live node
(the node doesn't hot-reload Rust — `make kill && make dev … CE=1`, or the publish flow) so the running
canvas picks up the fix. All code + tests are green.

---

## Issue 1 — dark/light mode doesn't work in the CE canvas  ⭐ ROOT CAUSE FOUND

**Symptom:** toggling the host Dark/Light switch doesn't re-theme the CE canvas; it stays dark.

**Root cause (confirmed):** a **class-name mismatch** between how the host sets the theme and how the
editor reads it.

- The host applies its theme by toggling a **`.dark`** class on `<html>` — light mode is the *absence*
  of `.dark`, not a `.light` class. See `ui/src/lib/theme/theme-dom.ts` (`applyThemePreference`) and its
  test `ui/src/lib/theme/theme-dom.test.ts` (`classList.contains("dark")` true/false; there is NO
  `.light` class).
- The editor's `useDocumentColorMode()` in `packages/ce-wiresheet/src/CeEditor.tsx:~221` reads:
  ```ts
  document.documentElement.classList.contains("light") ? "light" : "dark"
  ```
  Since the host never sets `.light`, this **always returns `"dark"`**.

**Fix (upstream in ce-wiresheet, S2):** make `useDocumentColorMode` agree with the host's convention —
`"dark"` when `.dark` is present, else `"light"`:
```ts
const read = () =>
  document.documentElement.classList.contains("dark") ? "dark" : "light";
```
The `MutationObserver` already watches `class` on `<html>`, so the toggle will track live. **Verify** the
editor's default (when neither class is present) matches the host's default mode.

**Watch out:** the editor's `.ce-wiresheet.theme-light` palette flip keys off `colorMode` → the
`theme-light` class on the editor root (`CeEditor.tsx:2835`). After slice-9.1, the editor's tokens now
also **inherit** the host's `--card`/`--fg`/… via `var(--token, default)`, so in the host the canvas
chrome should follow host light/dark *through the inherited tokens* even before this fix — but the
xyflow/node internal palette still branches on `colorMode`, so the class fix is still required. Test both:
host-light should give a light canvas AND light nodes.

---

## Issue 2 — CE header clashes with the host header

**Symptom (screenshot):** "Control Engine" + "· acme" from the CE page renders on TOP of the host's own
page header ("Lazybones / workspace ops" bar), overlapping text.

**Decision:** the **host header wins**. Remove the CE page's own `<header>` and surface only the
connected/disconnected badge (which links to the CE server-stats window).

**Where:**
- The CE page header is `rust/extensions/control-engine/ui/src/Page.tsx:50-70` (the
  `<header className="flex items-center gap-3 border-b border-border px-4 py-2">` block with the
  "Control Engine" title, `· {ctx.workspace}` subtitle, and the appliance `<select>`).
- **What to keep:** the appliance picker (`<select>`) still needs a home — move it into the editor's
  toolbar or a compact bar that does NOT duplicate the host's title/workspace. The host already shows the
  workspace, so drop `· {ctx.workspace}` and the "Control Engine" title.
- **The badge:** the connected/disconnected indicator lives in ce-wiresheet
  (`packages/ce-wiresheet/src/components/ConnectionStatus.tsx`) and is already rendered inside `CeEditor`.
  Confirm it's visible without the page header; it links to the CE server-stats/diagnostics window
  (`DiagDrawer`/`DiagPanel` in ce-wiresheet). If the page header was the only place the badge showed,
  wire `ConnectionStatus` into the editor's own top bar instead.

**Why it clashed:** the federated page renders INTO the host's content region, which already has the host
header above it. The CE page added a SECOND header of its own. This is a layout/composition decision, not
a CSS-leak (that's fixed) — the page should assume the host chrome owns the title bar.

---

## Issue 3 — node slots don't show inputs/outputs; values don't stream

**Symptom:** nodes on the canvas render without their input/output ports (slots), and live values don't
update.

**Status:** NOT yet root-caused — needs investigation. Likely coupled to issue #4 (see below): if the
`/nodes?withEdges=true` decode CRASHES, the editor never receives the component graph, so nodes have no
ports and no edges to stream along. **Re-check #3 AFTER #4 is fixed** — it may resolve or narrow.

**Where to look if it persists after #4:**
- Port rendering: `packages/ce-wiresheet/src/components/FunctionBlock.tsx` (the node component; ports come
  from the component's `properties` + the exposure spec). Check that `properties` is populated — recall
  the earlier `add-node` bug where the engine's response OMITTED `properties` and the store crashed
  (`docs/debugging/frontend/add-node-crashes-object-values-undefined-properties.md`); a similar
  lean-response shape could leave ports empty.
- Value streaming: `packages/ce-wiresheet/src/lib/subscriptions.ts` + the WS transport
  (`lib/transport-direct.ts` / the injected `BridgeTransport` on the LB side). The bridge transport's
  `openStream` was the subject of the prior detached-`this` crash — confirm the stream actually arms
  (the debugging entry `ce-page-crashes-openstream-detached-this-drops-shell.md` fixed a case where a
  remount skipped arm → static canvas + leaked sub). Verify frames arrive:
  `rust/extensions/control-engine/src/watch/` (pump/series/frame) is the LB-side stream source.
- The exposure/port model: `packages/ce-wiresheet/src/lib/exposure.ts` and `EXPOSURE_SPEC.md`.

**How to verify:** with a real CE appliance bound, open the browser devtools network tab — confirm the WS
`/ws` stream connects and frames flow, and that `/nodes?depth=1&withEdges=true` returns `properties` per
node. If the REST decode is failing (#4), you'll see the error toast and no graph at all.

---

## Issue 4 — codec crash: `missing field source_uid`  ⭐ ROOT CAUSE FOUND (the blank canvas)

**Symptom (red toast):**
```
extension error: child returned an error: bad host response: control-engine codec
error: /nodes?depth=1&withEdges=true: decoding response: missing field `source_uid`
at line 1 column 7041
```

**Root cause (confirmed):** the CE client crate **`rubix-ce`** (external git dep, pinned rev
`51ab97edf32d622f94d00401aee3ae2daf8859c8`, `github.com/NubeIO/ce-client-rust`) declares `EdgeDto` with
**required** `source_uid` and `target_uid`:
```rust
// ~/.cargo/git/checkouts/ce-client-rust-.../51ab97e/src/types.rs:268
pub struct EdgeDto {
    pub uid: u32,
    pub source_uid: u32,          // ← REQUIRED, no #[serde(default)]
    #[serde(default)] pub source_property_uid: Option<u32>,
    pub target_uid: u32,          // ← REQUIRED
    ...
}
```
The real CE appliance's `/nodes?depth=1&withEdges=true` response contains an edge object (at byte 7041)
that **omits `source_uid`** (probably a dangling / half-formed / cross-boundary edge, or a newer engine
that made the field optional). Serde fails the whole response decode → the extension returns "bad host
response" → the editor gets nothing → **blank canvas**.

**Where it's wired:** `rust/extensions/control-engine/src/engine.rs:12`
(`use rubix_ce::{CeRestClient, ControlEngine, …}`); Cargo pin at
`rust/extensions/control-engine/Cargo.toml:46`. The crate is an **external dep we can't edit in this
repo** — there's a commented `path = "/home/user/code/ce/ce-client-rust"` side-by-side option (line 47)
for local iteration.

**Fix options (pick based on where the contract should live):**
1. **Upstream (correct long-term):** make `source_uid`/`target_uid` `Option<u32>` with `#[serde(default)]`
   in `ce-client-rust`'s `EdgeDto`, decide what a null-endpoint edge MEANS (skip it? render as dangling?),
   cut a new rev, bump the pin in `Cargo.toml:46`. This is the real fix — the client must tolerate the
   engine's actual wire shape. Use the local `path =` checkout (line 47) to iterate, then re-pin.
2. **Extension-side tolerant decode (faster, if upstream is slow):** the extension can't change the
   struct, but it CAN intercept — fetch `/nodes` as raw JSON in the extension, drop/repair edges missing
   `source_uid`/`target_uid` before handing to `rubix-ce`'s typed layer, OR catch the decode error and
   return the node graph WITHOUT edges (degraded but non-blank). Look at where `engine.rs` /
   `src/tools/tree.rs` calls the client's node-tree method. **Caveat:** this masks a real data problem —
   prefer #1 unless you need the canvas up NOW.

**First: capture the offending payload.** Reproduce with the real appliance and dump the raw
`/nodes?depth=1&withEdges=true` JSON (curl the CE base directly, or log it in `engine.rs` before decode),
then look at byte ~7041 to see WHICH edge omits `source_uid` and WHY. That tells you whether the field is
legitimately optional (→ fix #1) or the engine is emitting a malformed edge (→ engine bug, different repo).

**Rule-9 test:** whichever fix, add a decode test with a REAL captured payload (a `/nodes` response that
includes a `source_uid`-less edge) asserting the extension no longer errors — seed it as a fixture from
the actual appliance response, not a hand-invented shape.

---

## Environment / build gotchas (carry forward)

- **Builds:** `packages/ce-wiresheet` (lib) must build BEFORE the ext UI (the ext aliases its `dist/`).
  ```
  cd packages/ce-wiresheet && pnpm run build:lib
  cd rust/extensions/control-engine/ui && ./node_modules/.bin/vite build
  ```
- **pnpm supply-chain policy blocks installs** — an unrelated lockfile entry (`@opencode-ai/sdk`) trips
  `minimumReleaseAge`. Work around: `pnpm install --config.minimumReleaseAge=0` and export
  `PNPM_CONFIG_MINIMUM_RELEASE_AGE=0` (the pre-run deps hook re-triggers it). The ext UI is a STANDALONE
  package: `pnpm install --ignore-workspace` in its dir. Pre-existing, flag for lockfile owner.
- **Rust build:** no system `cc` — links via zigcc (`rust/.cargo/config.toml`). The CE extension binary:
  `cargo build -p control-engine` (see `rust/extensions/control-engine/build.sh`).
- **The node binary does NOT hot-reload Rust** — after changing the extension, rebuild + republish (make
  kill && make dev). A stale node is a common "my fix didn't take" cause.
- **Live run:** `make dev EXTAGENT=1 DEVKIT_BUILDER=container CE=1` (needs `make docker-build-image` once
  for the container builder). `CE=1` sets `LB_CONTROL_ENGINE_BASE`/`_APPLIANCE`.

## Suggested order

1. **#4 codec crash** — unblocks the canvas. Capture the payload, decide optional-field vs engine bug,
   fix upstream in `ce-client-rust` (or tolerant-decode in the extension), re-pin, rebuild `-p
   control-engine`, republish.
2. **#3 node slots / streaming** — re-check after #4; likely partially caused by it. If it persists, walk
   `FunctionBlock` ports (`properties` populated?) + `subscriptions.ts`/WS frames.
3. **#1 theme** — one-line class fix in `useDocumentColorMode` (`.light`→`.dark` convention). Rebuild
   `build:lib` + ext UI.
4. **#2 header** — remove the CE page `<header>` (Page.tsx:50-70), rehome the appliance picker, surface
   the `ConnectionStatus` badge from the editor toolbar.

## Key files

| Concern | File |
| --- | --- |
| Theme read (bug) | `packages/ce-wiresheet/src/CeEditor.tsx` (`useDocumentColorMode`, ~L221) |
| Host theme convention | `ui/src/lib/theme/theme-dom.ts` (`.dark` toggle) |
| CE page header (remove) | `rust/extensions/control-engine/ui/src/Page.tsx:50-70` |
| Connection badge | `packages/ce-wiresheet/src/components/ConnectionStatus.tsx` |
| Node ports | `packages/ce-wiresheet/src/components/FunctionBlock.tsx`, `lib/exposure.ts` |
| Value streaming | `packages/ce-wiresheet/src/lib/subscriptions.ts`, `rust/extensions/control-engine/src/watch/` |
| Codec crash (EdgeDto) | `ce-client-rust` (git dep) `src/types.rs:268`; pin `control-engine/Cargo.toml:46` |
| Node-tree fetch | `rust/extensions/control-engine/src/engine.rs`, `src/tools/tree.rs` |
