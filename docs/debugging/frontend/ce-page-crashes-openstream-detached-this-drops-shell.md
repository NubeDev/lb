# Opening the Control-Engine page crashed the whole shell (`undefined reading 'bridge'` in `openStream`)

- **Area:** frontend
- **Date:** 2026-07-02
- **Status:** resolved
- **Symptom (as reported):** "the main UI sidebar is hidden when I click on the control-engine
  extension sidebar" — opening the CE extension page blanked the shell (nav + sidebar gone), and
  the console showed:
  - `Uncaught TypeError: Cannot read properties of undefined (reading 'bridge') at openStream (remoteEntry…)`
  - `Warning: Attempted to synchronously unmount a root while React was already rendering.`
  - `Uncaught NotFoundError: Failed to execute 'removeChild' on 'Node': The node to be removed is not a child of this node.`

## Root cause (two layers)

**Layer 1 — the crash: a detached method lost its receiver.** `CeEditor.tsx` called the injected
transport's `openStream` through a **bare local**:

```ts
const openStream = transport.openStream as (...) => EngineStream;  // detaches `this`
const stream = openStream(handlers, { ...presence });              // `this` === undefined inside
```

The control-engine `BridgeTransport.openStream` is an ordinary **prototype method** that reads
`this.bridge`. Extracting it to a local and calling it un-qualified makes `this` `undefined`, so the
first line of `openStream` (`this.bridge.watch`) threw `Cannot read properties of undefined (reading
'bridge')` the instant the editor mounted.

Why it was invisible to tests: the in-repo `MockTransport` in `transport.test.tsx` **arrow-binds** its
`openStream`/`request` (with a comment literally noting "the editor's casts may detach the method … keep
`this`"). The test was written *around* the bug, so the seam looked conformant while a real class-based
transport crashed in the browser.

**Layer 2 — the crash took down the SHELL, not just the page.** A federated extension renders
**in-process against the shell's React** (`ExtHost` → `mount(el, ctx, bridge)`). A render-time throw
inside the extension therefore unwinds the shell's fiber tree — hence the disappearing sidebar. Then
`ExtHost`'s effect cleanup ran `unmount()` + `el.replaceChildren()` synchronously while React was still
mid-commit → the "synchronously unmount a root" / "removeChild: not a child" secondary errors.

## Fixes

1. **`packages/ce-wiresheet/src/CeEditor.tsx`** — call `openStream` **as a method**
   (`transport.openStream(...)`), casting the *transport* (not the extracted function) so the receiver
   is preserved. Any `this`-reading transport now works.
2. **`packages/ce-wiresheet/src/CeEditor.tsx`** — the stream effect had **no cleanup**: the
   module-level `streamRef` (not a `useRef`) stayed populated after unmount, so the
   `if (streamRef.current) return` guard made a **remount skip arming** (canvas came back static, bus
   subscription leaked). Added a cleanup that `stream.close()`s and nulls `streamRef` on unmount /
   transport swap. (This is also what let two tests contaminate each other.)
3. **`ui/src/features/ext-host/ExtErrorBoundary.tsx`** (new) — a crash wall around the mounted
   extension, wired in `createAppRouter.tsx`. A render-time throw inside an extension now shows an
   honest inline error card and the **shell keeps rendering** (nav + sidebar survive). Keyed on the ext
   id so navigating to another extension re-arms it.
4. **`ui/src/features/ext-host/ExtHost.tsx`** — hardened the effect cleanup: `unmount()` and
   `el.replaceChildren()` are each wrapped so a throw during teardown (a page we're navigating away
   from) can never surface as React's "synchronously unmount a root" / "removeChild" errors.

## Regression test

`packages/ce-wiresheet/src/lib/transport.test.tsx` — new `ThisReadingTransport` whose `openStream`/
`request` are **prototype methods that dereference `this`** (mirroring the real `BridgeTransport`), and
a test asserting `CeEditor` mounts + arms the stream without throwing. Verified fail-before / pass-after:
with the detaching call restored the test throws `Cannot read properties of undefined (reading 'marker')`
— the exact shape of the production `reading 'bridge'`. Also reset the module-level rest singletons in
`afterEach` so tests are hermetic.

## Lessons

- A transport seam's methods can be called on a **class instance that reads `this`** — the caller must
  invoke them **as methods** (`obj.method(...)`), never via a detached local. Cast the object, not the
  extracted function.
- A conformance test whose test-double **arrow-binds** its methods hides exactly this class of bug. The
  double must match the *real* implementation's shape (prototype methods reading `this`) or it certifies
  a contract the real code violates — a rule-9-adjacent trap (the fake looks done; the real path crashes).
- A federated extension renders against the shell's React, so it **needs a crash wall** — otherwise any
  extension render throw blanks the whole app.
- An effect that writes a **module-level** ref must clean it up, or a remount silently skips the work the
  ref guards.
