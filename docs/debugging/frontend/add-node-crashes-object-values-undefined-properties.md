# Adding a node on the wiresheet threw `Cannot convert undefined or null to object`

- **Area:** frontend
- **Date:** 2026-07-02
- **Status:** resolved
- **Symptom (as reported):** on the control-engine canvas, **adding a new node** threw
  `Cannot convert undefined or null to object` — the add appeared to do nothing.

## Root cause

`Object.values(x)` throws `Cannot convert undefined or null to object` when `x` is `undefined`.

The add flow: `CeEditor.onAddNode` → `restAddNode()` (POST `/nodes`) returns the created component →
`useStructural.getState().upsertComponent(created)` → `indexComponentProperties(c)` →
`for (const p of Object.values(c.properties))` at **`packages/ce-wiresheet/src/lib/store.ts:44`**.

The engine's **add response** (POST `/nodes`) comes back **leaner** than a GET-loaded component: a
brand-new component has no ports materialized, so the response **omits `properties`** entirely. But the
`Component` type declares `properties: Record<string, Property>` as **non-optional**, so `rest.ts`
cast the raw response to `Component` with no normalization, and the store's bare
`Object.values(c.properties)` crashed on the missing map.

It reproduced **only on add**, never on load, because GET `/nodes` always returns the fully-hydrated
`properties`. And the store was the **odd one out**: every other consumer of `component.properties`
already guarded it (`paste.ts`, `choices.ts`, `FindPanel`, `InspectPanel`, `FunctionBlock` all use
`?? {}` / `?? []`) — only `store.ts:44` and its twin `store.ts:50` (`unindexComponentProperties`,
hit on a re-upsert) did a bare `Object.values`.

## Fix (two layers)

1. **Boundary normalization (primary, correct fix)** — `packages/ce-wiresheet/src/lib/rest.ts`. A new
   `normalizeComponent(c)` restores the type's promised invariant (`properties ?? {}`) **once**, at the
   client boundary, applied to every endpoint that returns a bare `Component` and can omit it:
   `addNode`, `updateNode`, `patchOverrides`, `groupComponents`. The returned value now matches the GET
   shape, so no downstream consumer needs a defensive guard. No-op for GET (already carries `properties`).
2. **Store hardening (defense-in-depth)** — `packages/ce-wiresheet/src/lib/store.ts`. `index`/`unindex`
   ComponentProperties use `Object.values(c.properties ?? {})` so a component reaching the store from
   **any** source (a non-REST transport, a test seed, a future endpoint) can never crash the index.

## Regression test

`packages/ce-wiresheet/src/lib/store.test.ts` — "upsertComponent tolerates a component with NO
properties (add-response shape)": upserts a component missing `properties` (the raw add-response shape)
and asserts no throw, plus a re-upsert (which exercises `unindexComponentProperties`). Verified
fail-before / pass-after: reverting the store guard makes it throw the exact reported
`Cannot convert undefined or null to object`.

## Lessons

- A **mutating** endpoint's response can be leaner than the **read** endpoint's — normalize it to the
  read shape **once, at the fetch boundary**, so the rest of the client can trust the type's invariant
  rather than sprinkling `?? {}` at every consumer (and missing one, as the store did).
- When a type says a field is non-optional but the wire can omit it, the type is lying — the boundary
  that decodes the wire owns making the type true.
- `Object.keys/values/entries(x)` throws on `null`/`undefined` — any of them fed a field the wire can
  omit is a latent `Cannot convert undefined or null to object`.
