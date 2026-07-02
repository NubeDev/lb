# CE engine — REST/WS API requests

We control the control-engine's REST/WS surface, so the client should **read state
the engine produces** rather than reconstruct it. This file lists engine changes
that let us delete client-side workarounds. Each item: the **request**, the
**workaround it removes**, and **why the engine is the right owner**.

Priority order is roughly by how much client complexity each removes.

---

## 1. Property → component owner — ✅ LANDED (engine); now resolve edge owners from the prop uid

**Status: the per-property owner is LANDED.** `componentUid` is now intrinsic on
every `PropertyShm` (set write-once at creation, fits the existing tail padding so
the SHM ABI is unchanged), surfaced through `IPropertyData::componentUid()` and the
serializers — so **any** prop uid now answers "who owns me?" in O(1) via the prop
pool. (`hasPropertyUid` dropped to an O(1) compare as a bonus.) This inverts the
old one-directional component→prop model; the earlier "store the owner on the
exposure record" idea is no longer needed — the global owner subsumes it.

**Now do (engine, small — the part that actually deletes the client workaround).**
`POST /edge` should resolve the owning component from the source/target **prop
uid** (resolve the prop in the pool → read `componentUid`), so a new edge to a
folder port can be posted with the **port's prop uid alone** and the engine fills
in the owner. Today, posting the folder uid still 400s — "Unable to find property
`<folder>.<prop>`" — because the prop lives on a deep child, not the folder.

**Frontend cleanup this unlocks** (separate repo — do once the edge endpoint
resolves prop→owner):

- delete `lib/routing.ts::buildPropOwners` + the deep-subtree prop→owner walk;
- delete the `exposedRemap` override in `reload` (it existed only to retarget a new
  edge from a folder port — esp. a *chained* port — to its real deep owner);
- `onConnect` posts the two handle prop uids; the engine resolves the owners.

**Why this shape.** The owner is a field on the prop slot (write-once, no sync — a
property never reparents), not a separate 80 KB reverse index. Resolving the edge
owner server-side keeps the deep-subtree knowledge where it belongs and lets the
client post the handle it already has.

---

## 2. Subtree-scoped, view-classified edges — ✅ LANDED (Part A + Part B)

**Shipped on the engine** (`GET /edges?subtree=<uid|path>`, verified in the live
`openapi.yaml`). Each `Edge` in the response carries `class`
(`internal` / `boundary` / `external`) + `sourceContainer` / `targetContainer` (the
direct-child uid each endpoint resolves under; `0` = the view root; **absent** =
outside the view). `internal` = both ends in the same child folder (the loopback
case). Exactly Part A + Part B, including the recommendation to stop at containers.

**Client cleanup (this repo):** replace the `DEEP_EDGE_DEPTH = 32` deep node
refetch with one `GET /edges?subtree=`; drop `class === "internal"` edges; delete
`lib/routing.ts::buildContainerOf` and the container param on `partitionEdges`.
Bonus: it also fixes the deep-to-deep under-exposure (those edges are now fetched).

The original request follows for reference.

---

`collectEdges` is coupled to the node-walk depth, so a `depth=1` read of a
folder-of-folders returns **zero** edges (the real edges live among grandchildren).
Two parts; **both** are needed for the client to fully collapse — see the coupling.

### Part A — return every edge touching the subtree → **new endpoint (Option 1)**

`GET /edges?subtree=<uid|path>` → `collectEdges(root, depth=-1, visibleOnly=false)`.
`collectEdges` already recurses unboundedly at `depth=-1`, and each member's
`edgesIn`+`edgesOut` naturally captures every edge with ≥1 endpoint inside. Additive,
zero risk to existing callers.

Prefer this over decoupling `withEdges` (Option 2): the new endpoint is the natural
home for the view-relative classification below, and `reload` already does a 2-call
pattern guarded by a reload generation, so a separate edges call costs nothing in
consistency. Either way it kills the `DEEP_EDGE_DEPTH = 32` refetch.

### Part B — view-relative classification → **required, not optional**

Emit per edge: `class` (`internal` / `boundary` / `external`) +
`sourceContainer` / `targetContainer` (the direct-child uid each end resolves under;
`0` = root-level, absent = outside the view). Built by tagging every descendant of
each direct child `Ci` with `Ci` (one O(subtree) pass), then classifying by
(srcContainer, tgtContainer): same `Ci` → internal (the folder-loopback case);
different in-view containers → boundary; one end outside → external.

**Why B is not gold-plating — the coupling.** Today `lib/routing.ts::buildContainerOf`
rides on the **deep NODE** fetch (it needs the parent hierarchy). §1 removes
`buildPropOwners` — the *other* reason the client pulls the deep node tree. So once
§1 lands, the ONLY remaining reason the client fetches the deep node tree is to
build `containerOf`. If Part A returns **edges only**, the client must *still* fetch
the deep node tree for containers — no win. **Part B is exactly what lets the client
stop fetching the deep node tree at all.** A alone mostly doesn't collapse anything;
A+B does.

**Stop at containers — don't emit the port handle.** The client keeps the children's
`__facets` from the depth-1 read (it renders the port rows from them anyway), so it
maps `(container, prop) → port handle` itself. Containers are the clean engine/client
boundary; emitting rendering anchors would couple the engine to the client's facet
projection.

**Removes.** `DEEP_EDGE_DEPTH` refetch, `lib/routing.ts::buildContainerOf`, and the
same-container drop in `partitionEdges`. **Keeps:** `exposedPortIndex` (port handles
from facets) and a slim `classifyCrossEdge` that reads the engine's `*Container`
fields instead of reconstructing them.

---

## 3. The engine should MAINTAIN folder exposure (`__facets`) on every topology change

**Request.** The Folder/container component should recompute its exposed-port
records (`computeFolderExposure`, EXPOSURE_SPEC) on **every** change that affects a
boundary: edge add/delete, reparent in/out, child delete, group/ungroup — not only
when the folder itself is edited. Then `__facets` is always correct and the client
only **reads** exposure.

**Removes.** A large amount of optimistic client recomputation:

- `lib/grouping.ts::groupBoundary`, `groupChainedBoundary`, `rechainParentAfterGroup`
- `pruneExposure` + the prune hooks in `onNodesDelete` and the move-into picker
- the periodic "stale `__facets`" problems we keep hitting: a dangling `#<uid>`
  port after a crossing edge is deleted, an ancestor that still points directly at
  a child after a nested group, a folder that never chained a member's port.

**Why.** Exposure is a pure function of (hierarchy, edges). The engine mutates both
and already has the algorithm. The client mirroring it is duplicated logic that
drifts out of sync (every stale-record bug in this area is a sync gap).

---

## 4. A server-side group / ungroup operation

**Request.** `POST /group { uids: [...] }` → engine creates the folder, reparents
the members, computes the new folder's exposure **and** re-chains the old parent,
atomically; `POST /ungroup { uid }` does the inverse. (Subsumes §3 for the group
gesture.)

**Removes.** The whole client grouping pipeline in `CeEditor.tsx::groupSelected`
(add folder → bulk reparent → write boundary facets → re-chain parent), which is
currently 3–4 separate writes the client has to compute and order.

**Why.** Grouping is one logical operation; doing it as several client writes is
non-atomic (a partial failure leaves a half-grouped tree) and re-implements the
engine's exposure algorithm.

---

## 5. `/copy/nodes` should remap `__facets` uid references

**Request.** `/copy/nodes` already mints new uids and returns the `uidMap`. It
should **apply that map to copied `__facets` values** (record key = the deep prop
uid, plus `childComponent` / `facetProp`) before returning, so a pasted container's
ports point at *its own* children.

**Removes.** `lib/facet.ts::remapFacetUids` and the facet-remap half of
`lib/paste.ts::planPaste`.

**Why.** The engine copies the facet string verbatim, so it still references the
original uids; only the engine has the authoritative `uidMap` at copy time. (This
is the `§0a` referenced from `engine-types.ts` / `paste.ts`.)

---

## 6. Exposure records should carry the child property's name

**Request.** When the engine writes an exposed-port record, include the child
property **name** (or a default label). Today the record has `expose`,
`childComponent`, `facetProp`, `chain` — but **no name**, so an engine-written port
renders as `#<propUid>` until the client back-derives the name.

**Removes.** The `#<uid>` fallback and the need to recover a port's name from the
crossing edge / `FunctionBlock.rowIndexOf` plumbing for labels.

**Why.** The engine knows the child prop's name; the UI shouldn't reverse-engineer
it from edges.

---

## 7. `POST /edge` should return the FULL created edge (incl. `loopBack` + prop uids) — ✅ LANDED

**Status: LANDED — verified on the live engine (`:7878`).** The create response now
returns the full `Edge` object (the `GET /edges` shape), including `loopBack` and the
prop/component uids:

```jsonc
// POST /api/v0/edge { sourceUid:100007, sourcePropUid:1000071,   // out
//                     targetUid:100007, targetPropUid:1000074 }   // in1 (self-edge)
{ "data": {
  "uid": 1000000,
  "sourceUid": 100007, "sourcePropertyUid": 1000071, "sourceProperty": "out",
  "targetUid": 100007, "targetPropertyUid": 1000074, "targetProperty": "in1",
  "loopBack": true, "hidden": false,
  "sourcePath": "root/loopCheck-1", "targetPath": "root/loopCheck-1"
} }
```

**Effect — no client change needed.** The client already reads `created.loopBack`
(dotted style) and spreads `...created` into its store, so:

- a freshly-drawn feedback edge (`out→in1`, or anything that closes a cycle) now
  renders **dotted immediately** — and, because the store record carries `loopBack`,
  it also dashes correctly as a **ghost** across a folder boundary; and
- `sourcePropertyUid`/`targetPropertyUid` are now echoed, so the client's "force the
  prop uids onto the stored edge" step in `onConnect`/`connectEdge` and the name→uid
  fallback in `lib/grouping.ts::groupBoundary` are **now redundant** and can be dropped
  as a cleanup (harmless either way — the forced values match the response).

**Remaining (minor): request field casing.** `POST /edge` (and our `EdgeRequest`)
still takes **`loopback`** (lowercase b) while the stored/`GET`/`PATCH` edge uses
**`loopBack`** (camel B). Accepting `loopBack` on the request too would give one
spelling across add / read / update.

---

## 8. (minor) camelCase/snake_case consistency

The engine emits a mix of `componentUid`/`statusFlags`/`systemRole`/`typeId`
(camelCase) and snake_case across endpoints; the proxy DTOs carry both via
`#[serde(alias = ...)]` (`crates/rubix-ce/src/types.rs`). Pick one casing for the
wire format and the aliases can go.

---

### What this session added that these would simplify

This session's work leans on §1, §2, and the loopback fix on §2/§3:

- the **chained-port wiring fix** (`buildPropOwners`) is a stand-in for **§1** — and
  with the property-owner field now landed, it disappears the moment `POST /edge`
  resolves the owner from the prop uid (then `onConnect` just posts the handle);
- the **cross-folder ghost rendering** needs the **§2** deep edge refetch;
- the **internal-loopback drop** (`buildContainerOf`) is reconstructing subtree
  membership the engine could return per **§2**;
- the **grouping/prune/re-chain** correctness work is the client mirroring **§3/§4**.

§1's engine half is in. Land its `POST /edge` resolution + §2, and the client's
edge layer collapses to "read the edges the engine returns, post the two handle
prop uids" — no owner resolution, no deep refetch, no container index.
