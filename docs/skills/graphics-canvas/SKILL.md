---
name: graphics-canvas
description: >-
  Draw and edit Lazybones plant-graphics / floor-plan scenes over the node gateway — the free-form
  three.js canvas the `thecrew` (Graphics) extension renders. Use when a task says "draw an AHU / plant
  graphic / mimic / floor plan", "make a scene", "add/bind equipment to live series", or "edit the
  graphics page over the API". A scene is a workspace DOCUMENT you read-modify-save with the shipped
  `assets.*` verbs — no canvas-specific verb exists. Covers the scene schema, the shape catalog, the
  `bind → series` contract, the read-modify-save loop, and a worked "draw AHU-1" run.
---

# Drawing graphics-canvas scenes over MCP

A **scene** is a declarative JSON document describing a graphics page — shapes, transforms, and data
bindings. The `thecrew` extension renders it with three.js (flat top-down for plant graphics; the same
document goes 3D with `camera:"persp"` later). **There is NO canvas verb** — a scene is stored and read
through the shipped generic document verbs, so drawing == editing a document:

```
scene document  ──assets.put_doc──▶  SurrealDB (workspace-walled)  ──assets.get_doc──▶  <SceneCanvas>
   you write it (read-modify-save)                                      every open canvas re-renders
```

Everything is capability-gated server-side and workspace-walled: the workspace + principal come from the
**bearer token**, never the request body. Bindings render under the **viewer's** grant — an
admin-authored scene never widens a viewer.

## 1. Authenticate

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login \
  -H 'content-type: application/json' -d '{"user":"user:ada","workspace":"acme"}' | jq -r .token)
```

Send `Authorization: Bearer $TOKEN` on every call. Capabilities you need (the `thecrew` install grant +
member set carry these): `mcp:assets.get_doc:call`, `mcp:assets.put_doc:call`, `mcp:assets.list_docs:call`.
A denial is **opaque** (you can't tell forbidden from absent) — if a save 403s, the caller lacks
`assets.put_doc`.

## 2. The verbs (all via the universal MCP bridge)

`POST /mcp/call {tool, args}` — the same chokepoint the UI and other agents use (rule 7).

| Action | `tool` | `args` |
|---|---|---|
| List scenes | `assets.list_docs` | `{}` → `{docs:[{id,title}]}` (filter ids starting `scene:`) |
| Read a scene | `assets.get_doc` | `{id}` → `{content}` (the scene JSON as a string) |
| Save a scene | `assets.put_doc` | `{id,title,content,content_type:"json",tags:["scene"],ts}` |

**Conventions (not enforced by the store — the extension relies on them):**
- Scene id is prefixed **`scene:`** (e.g. `scene:ahu-1`). `list_docs` returns only `{id,title}` (no
  tags), so the prefix IS how the picker discovers scenes. Always tag `scene` too (for a future
  tag-returning list).
- `content` is the scene JSON **as a string** (the doc store holds text); `content_type:"json"`.
- `ts` is a millisecond clock int (any monotonic value; the demo uses `1`).

> **No revision check.** `put_doc` is last-writer-wins today. To edit safely: `get_doc` first, modify the
> parsed content, `put_doc` back — and if two writers race, the last save wins (a generic
> `document-store/` revision check is a separate ask). Don't blind-overwrite a scene you didn't read.

## 3. The scene schema

```jsonc
{
  "v": 1,
  "camera": "ortho-top",          // flat plant graphics; "persp" for 3D (phase 5)
  "bg": { "asset": "floor.svg" }, // optional underlay (omit for equipment scenes)
  "shapes": {                     // a FLAT id → shape map (easy to patch incrementally)
    "sf1": {
      "type": "hvac.fan",         // MUST be a catalog type (§4) — unknown → placeholder box + a teaching error
      "t": { "x": 96, "y": 0, "r": 0, "sx": 1, "sy": 1 },  // transform; only x,y required (default 0)
      "props": { "diameter": 72, "direction": "right", "label": "SF-1" },
      "bind": {                   // prop ← live series. Each slot: { "channel": "<series name>" }
        "running": { "channel": "ahu1.sf1.running" },
        "speed":   { "channel": "ahu1.sf1.speed" }
      }
    }
  }
}
```

**The `bind` contract is the important part:** `bind[slot] = { "channel": "<series>" }`, where the channel
is a **series name** the node ingests. The extension's one multiplexer backfills each series via
`series.latest` and streams it via `series.watch`. Bind ONLY the slots a shape declares (§4) — a bind to
an unknown slot is dropped with a teaching error. To see what series exist, the operator ingests them;
you reference them by name.

Coordinates: ground plane is XY, origin at center, +x right / +y up, units are world units (~pixels at
default zoom). Lay a train of equipment left→right along y=0 spaced ~64 apart (see the worked run).

## 4. The shape catalog

Use one of these `type` values. `props(kind)` are authorable; `bind:` lists the slots that accept a
`{channel}`. (This list is generated from the renderer's registry — a scene using anything else renders a
labeled placeholder AND the validator returns a teaching error citing this catalog.)

```
hvac.duct — props: label(text), width(number), medium(select: air|chw|hw); bind: flow
hvac.fan — props: label(text), diameter(number), direction(select: left|right); bind: running, speed, fault
hvac.damper — props: label(text), width(number), actuated(boolean); bind: position
hvac.filter — props: label(text), width(number), stages(number); bind: dp, fault
hvac.coil — props: label(text), width(number), medium(select: chw|hw); bind: valve, temp_in, temp_out
hvac.casing — props: name(text), w(number), h(number); bind: status
plan.wall — props: label(text), thickness(number)
plan.room — props: name(text), w(number), h(number); bind: temp, occupied
plan.door — props: label(text), width(number), swing(select: left|right)
plan.label — props: text(text), size(number); bind: value
```

Notes: `hvac.duct` and `plan.wall` also take a `points` prop — an array of `[x,y]` in shape-local
coords — for the run/segment geometry (e.g. `"points": [[-160,0],[160,0]]`). `hvac.casing` is the AHU
outline you place equipment *inside*. `plan.label` bound to `value` shows a live readout.

## 5. The read-modify-save loop (how you actually draw)

1. **Read** (or start fresh): `assets.get_doc {id:"scene:ahu-1"}` → parse `content`. A new scene starts
   `{"v":1,"camera":"ortho-top","shapes":{}}`.
2. **Modify** the `shapes` map — add/patch/remove entries by id. Incremental patches are fine (the map is
   flat by design).
3. **Save**: `assets.put_doc {id, title, content: <JSON string>, content_type:"json", tags:["scene"], ts}`.
4. **Verify + self-correct**: read it back and check it parses; if the canvas shows placeholder boxes,
   the type was wrong — the extension's validator (`teachingReport`) names the failing shape and prints
   the catalog above. Pick a real type and re-save. **Never leave a scene with unknown types.**

Draw in a few passes for a page that "draws itself" as each save re-renders: (a) casing + ducts + underlay,
(b) equipment shapes with `bind`, (c) labels.

## 6. Worked run — "draw AHU-1"

Goal: *outside-air damper → filter → cooling coil → supply fan SF-1, in an AHU casing, bound to
`ahu1.*`, with a supply-air duct.*

```bash
CALL() { curl -s -X POST http://127.0.0.1:8080/mcp/call \
  -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"tool\":\"$1\",\"args\":$2}"; }

# The scene doc (build it in your language of choice; shown inline here).
read -r -d '' SCENE <<'JSON'
{
  "v": 1, "camera": "ortho-top",
  "shapes": {
    "casing": { "type": "hvac.casing", "t": {"x":0,"y":0}, "props": {"w":320,"h":128,"name":"AHU-1"},
                "bind": {"status": {"channel":"ahu1.sf1.running"}} },
    "duct-int": { "type": "hvac.duct", "t": {"x":0,"y":0},
                  "props": {"points":[[-160,0],[160,0]],"width":40,"medium":"air"},
                  "bind": {"flow": {"channel":"ahu1.sf1.speed"}} },
    "oad":    { "type": "hvac.damper", "t": {"x":-120,"y":0}, "props": {"width":48,"label":"OA damper"},
                "bind": {"position": {"channel":"ahu1.oad.position"}} },
    "filter": { "type": "hvac.filter", "t": {"x":-56,"y":0}, "props": {"width":48,"label":"Filter"},
                "bind": {"dp": {"channel":"ahu1.filter.dp"}} },
    "coil":   { "type": "hvac.coil", "t": {"x":8,"y":0}, "props": {"width":48,"medium":"chw","label":"CHW coil"},
                "bind": {"valve": {"channel":"ahu1.chwv.valve"}, "temp_out": {"channel":"ahu1.sat"}} },
    "sf1":    { "type": "hvac.fan", "t": {"x":96,"y":0}, "props": {"diameter":72,"direction":"right","label":"SF-1"},
                "bind": {"running": {"channel":"ahu1.sf1.running"}, "speed": {"channel":"ahu1.sf1.speed"},
                         "fault": {"channel":"ahu1.sf1.fault"}} }
  }
}
JSON

# content must be the scene JSON AS A STRING — jq -Rs quotes it into the put_doc args.
ARGS=$(jq -n --arg c "$SCENE" '{id:"scene:ahu-1",title:"AHU-1",content:$c,content_type:"json",tags:["scene"],ts:1}')
CALL assets.put_doc "$ARGS"

# verify
CALL assets.get_doc '{"id":"scene:ahu-1"}' | jq -r .content | jq .shapes.sf1
```

Any open Graphics canvas (page or a dashboard `ext:thecrew/scene` cell bound to `scene:ahu-1`) redraws
on the save, with live values on the shapes (SF-1's speed spins its impeller, the coil shows SAT, etc.).

## 7. Embed a scene in a channel / dashboard

A scene renders in a dashboard cell (and a channel rich-response) via the shipped ext-widget path — **no
new surface**:

```jsonc
{ "view": "ext:thecrew/scene", "options": { "sceneId": "scene:ahu-1" } }
```

In a dashboard this is a `[[widget]]` cell (add it in the builder: *Add panel → source "thecrew · Scene"
→ pick the scene*, or seed the cell shape above via `dashboard.save`). The widget is **read-only** (its
grant omits `put_doc`) — it renders the scene + live values, framed to the cell, and can never save.

## 8. Capability + isolation notes

- **Save denied** → the caller lacks `mcp:assets.put_doc:call`. Reads (`get_doc`/`list_docs`) and saves
  are separate caps; the read-only widget grant deliberately omits the write + list caps.
- **Workspace wall** — a scene saved in ws A is invisible from ws B through the same verbs; a series bound
  in a scene is only readable by a viewer whose token is in that workspace and holds `series.read`/`watch`.
  A bound shape a viewer can't read renders its no-access (null) state — never a crash.
- **Bindings run under the VIEWER's grant**, not the author's — you cannot draw a scene that leaks series
  a viewer isn't granted.

## Related

- Extension + phases: `rust/extensions/thecrew/docs/thecrew-extension-scope.md`; parent design:
  `docs/scope/frontend/graphics-canvas-scope.md`.
- The catalog + teaching validator live in `rust/extensions/thecrew/ui/src/scene/{catalog,validate}.ts`
  (this doc's §4 is generated from that registry — keep them in sync).
- Assets verbs: `docs/skills/store-read/SKILL.md` (the document surface); dashboards:
  `docs/skills/dashboard-mcp/SKILL.md` (the embed cell).
